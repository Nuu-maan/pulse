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
    let main = counts
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| **c)
        .map(|(l, _)| l)
        .unwrap_or(0);
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
