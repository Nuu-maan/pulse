use crate::repo::Commit;
use std::collections::HashMap;

pub struct Edge {
    pub child: usize,
    pub parent: Option<usize>,
}

pub struct Layout {
    pub lane_of: Vec<usize>,
    pub lanes: usize,
    pub edges: Vec<Edge>,
}

pub fn compute(commits: &[Commit]) -> Layout {
    let n = commits.len();
    let index: HashMap<&str, usize> = commits
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), i))
        .collect();

    let mut lane_of = vec![0usize; n];
    let mut slots: Vec<Option<String>> = Vec::new();
    let mut edges = Vec::new();

    for i in (0..n).rev() {
        let c = &commits[i];
        let mine: Vec<usize> = slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.as_deref() == Some(c.id.as_str()))
            .map(|(j, _)| j)
            .collect();

        let lane = match mine.first() {
            Some(&j) => j,
            None => alloc(&mut slots),
        };
        for &j in mine.iter().skip(1) {
            slots[j] = None;
        }
        slots[lane] = None;
        lane_of[i] = lane;

        for (pi, p) in c.parents.iter().enumerate() {
            match index.get(p.as_str()) {
                Some(&po) => {
                    edges.push(Edge {
                        child: i,
                        parent: Some(po),
                    });
                    if slots.iter().any(|s| s.as_deref() == Some(p.as_str())) {
                        continue;
                    }
                    if pi == 0 && slots[lane].is_none() {
                        slots[lane] = Some(p.clone());
                    } else {
                        let j = alloc(&mut slots);
                        slots[j] = Some(p.clone());
                    }
                }
                None => edges.push(Edge {
                    child: i,
                    parent: None,
                }),
            }
        }
    }

    let lanes = lane_of.iter().copied().max().unwrap_or(0) + 1;
    promote_trunk(&mut lane_of, lanes);

    Layout {
        lane_of,
        lanes,
        edges,
    }
}

fn promote_trunk(lane_of: &mut [usize], lanes: usize) {
    let mut counts = vec![0usize; lanes];
    for &l in lane_of.iter() {
        counts[l] += 1;
    }
    let mut main = 0;
    for (lane, &count) in counts.iter().enumerate() {
        if count > counts[main] {
            main = lane;
        }
    }
    if main == 0 {
        return;
    }
    for l in lane_of.iter_mut() {
        if *l == 0 {
            *l = main;
        } else if *l == main {
            *l = 0;
        }
    }
}

fn alloc(slots: &mut Vec<Option<String>>) -> usize {
    match slots.iter().position(|s| s.is_none()) {
        Some(j) => j,
        None => {
            slots.push(None);
            slots.len() - 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(id: &str, parents: &[&str]) -> Commit {
        Commit {
            id: id.into(),
            short: id.into(),
            parents: parents.iter().map(|p| p.to_string()).collect(),
            author: "Test".into(),
            email: "test@example.com".into(),
            time: 0,
            summary: String::new(),
            refs: Vec::new(),
            ins: 0,
            del: 0,
        }
    }

    fn chain(ids: &[&str]) -> Vec<Commit> {
        ids.iter()
            .enumerate()
            .map(|(i, id)| {
                if i == 0 {
                    commit(id, &[])
                } else {
                    commit(id, &[ids[i - 1]])
                }
            })
            .collect()
    }

    #[test]
    fn linear_history_stays_on_one_lane() {
        let l = compute(&chain(&["a", "b", "c", "d"]));
        assert_eq!(l.lanes, 1);
        assert_eq!(l.lane_of, vec![0, 0, 0, 0]);
        assert_eq!(l.edges.len(), 3);
    }

    #[test]
    fn a_branch_and_merge_take_two_lanes() {
        let commits = vec![
            commit("a", &[]),
            commit("b", &["a"]),
            commit("f", &["a"]),
            commit("m", &["b", "f"]),
        ];
        let l = compute(&commits);
        assert_eq!(l.lanes, 2);
        assert_eq!(l.lane_of[3], 0, "the merge belongs on the trunk");
        assert_eq!(l.edges.len(), 4);
    }

    #[test]
    fn a_parent_outside_the_window_becomes_a_stub() {
        let commits = vec![commit("b", &["a"]), commit("c", &["b"])];
        let l = compute(&commits);
        let stubs = l.edges.iter().filter(|e| e.parent.is_none()).count();
        assert_eq!(stubs, 1);
    }

    #[test]
    fn the_busiest_lane_is_promoted_to_the_top() {
        let mut commits = chain(&["root", "m1", "m2", "m3", "m4", "m5"]);
        commits.push(commit("f1", &["m1"]));
        let l = compute(&commits);

        assert_eq!(l.lanes, 2);

        let mut counts = vec![0usize; l.lanes];
        for &lane in &l.lane_of {
            counts[lane] += 1;
        }
        assert!(
            counts[0] > counts[1],
            "the top lane should be the busiest, got {counts:?}"
        );
        assert_eq!(l.lane_of[6], 1, "the side branch sits below the trunk");
    }

    #[test]
    fn an_empty_history_does_not_panic() {
        let l = compute(&[]);
        assert_eq!(l.lanes, 1);
        assert!(l.edges.is_empty());
    }
}
