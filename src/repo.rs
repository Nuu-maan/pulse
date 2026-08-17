use anyhow::{Context, Result};
use git2::{Repository, Sort};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct Commit {
    pub id: String,
    pub short: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    pub time: i64,
    pub summary: String,
    pub refs: Vec<String>,
    pub ins: usize,
    pub del: usize,
}

pub struct History {
    pub name: String,
    pub head: String,
    pub commits: Vec<Commit>,
    pub truncated: bool,
    pub stats: bool,
}

pub struct Query<'a> {
    pub rev: Option<&'a str>,
    pub all: bool,
    pub max: usize,
    pub since: Option<i64>,
    pub stats: bool,
}

impl History {
    pub fn authors(&self) -> usize {
        self.commits
            .iter()
            .map(|c| c.email.to_lowercase())
            .collect::<HashSet<_>>()
            .len()
    }

    pub fn span(&self) -> Option<(i64, i64)> {
        let first = self.commits.first()?;
        let last = self.commits.last()?;
        Some((first.time, last.time))
    }

    pub fn churn(&self) -> (usize, usize) {
        self.commits
            .iter()
            .fold((0, 0), |(i, d), c| (i + c.ins, d + c.del))
    }
}

pub fn load(path: &Path, q: &Query) -> Result<History> {
    let repo = Repository::discover(path)
        .with_context(|| format!("no git repository at {}", path.display()))?;

    let name = repo
        .workdir()
        .unwrap_or_else(|| repo.path())
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repository".into());

    let head = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().ok().map(String::from))
        .unwrap_or_else(|| "HEAD".into());

    let refs = collect_refs(&repo);

    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    match (q.rev, q.all) {
        (Some(rev), _) => {
            let obj = repo
                .revparse_single(rev)
                .with_context(|| format!("cannot resolve revision `{rev}`"))?;
            walk.push(obj.peel_to_commit()?.id())?;
        }
        (None, true) => {
            walk.push_glob("refs/heads/*")?;
            let _ = walk.push_glob("refs/tags/*");
        }
        (None, false) => {
            if repo.head().is_ok() {
                walk.push_head()?;
            } else {
                walk.push_glob("refs/heads/*")?;
            }
        }
    }

    let mut commits = Vec::new();
    let mut truncated = false;
    for oid in walk {
        let oid = oid?;
        if commits.len() >= q.max {
            truncated = true;
            break;
        }
        let c = repo.find_commit(oid)?;
        let time = c.time().seconds();
        if let Some(cutoff) = q.since {
            if time < cutoff {
                truncated = true;
                break;
            }
        }
        let author = c.author();
        let id = oid.to_string();
        let (ins, del) = if q.stats {
            churn_of(&repo, &c).unwrap_or((0, 0))
        } else {
            (0, 0)
        };
        commits.push(Commit {
            ins,
            del,
            short: id[..7.min(id.len())].to_string(),
            parents: c.parent_ids().map(|p| p.to_string()).collect(),
            author: author.name().unwrap_or("unknown").to_string(),
            email: author.email().unwrap_or("").to_string(),
            time,
            summary: c.summary().ok().flatten().unwrap_or("").to_string(),
            refs: refs.get(&id).cloned().unwrap_or_default(),
            id,
        });
    }

    if commits.is_empty() {
        anyhow::bail!("no commits found in {}", path.display());
    }

    commits.reverse();
    Ok(History {
        name,
        head,
        commits,
        truncated,
        stats: q.stats,
    })
}

fn churn_of(repo: &Repository, c: &git2::Commit) -> Result<(usize, usize)> {
    let new_tree = c.tree()?;
    let old_tree = match c.parent(0) {
        Ok(p) => Some(p.tree()?),
        Err(_) => None,
    };
    let mut opts = git2::DiffOptions::new();
    opts.ignore_filemode(true).context_lines(0);
    let diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts))?;
    let s = diff.stats()?;
    Ok((s.insertions(), s.deletions()))
}

fn collect_refs(repo: &Repository) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    let Ok(refs) = repo.references() else {
        return out;
    };
    for r in refs.flatten() {
        if r.is_remote() {
            continue;
        }
        let Ok(name) = r.shorthand() else { continue };
        if name == "HEAD" {
            continue;
        }
        let Ok(commit) = r.peel_to_commit() else {
            continue;
        };
        out.entry(commit.id().to_string())
            .or_default()
            .push(name.to_string());
    }
    for v in out.values_mut() {
        v.sort();
        v.dedup();
        v.truncate(2);
    }
    out
}
