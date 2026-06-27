// src/git/graph.rs
use git2::{Repository, Oid};
use std::collections::HashMap;
use std::path::PathBuf;

// --- Data types ---

#[derive(Debug, Clone)]
pub struct CommitNode {
    pub oid: String,
    pub summary: String,
    pub author: String,
    pub time: i64, // unix timestamp
    pub parent_oids: Vec<String>,
    pub refs: Vec<RefLabel>,
}

#[derive(Debug, Clone)]
pub struct RefLabel {
    pub name: String,
    pub kind: RefKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RefKind {
    Branch,
    RemoteBranch,
    Tag,
    Head, // currently checked-out
}

#[derive(Debug, Clone)]
pub struct PositionedCommit {
    pub node: CommitNode,
    pub row: usize,  // y position (0 = newest)
    pub lane: usize, // x position (column in graph)
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from_row: usize,
    pub from_lane: usize,
    pub to_row: usize,
    pub to_lane: usize,
}

#[derive(Debug, Clone)]
pub struct PositionedGraph {
    pub commits: Vec<PositionedCommit>,
    pub edges: Vec<GraphEdge>,
    pub lane_count: usize,
}

// --- Loading ---

pub fn load(path: PathBuf) -> Result<PositionedGraph, git2::Error> {
    let repo = Repository::open(&path)?;
    let nodes = walk_commits(&repo)?;
    Ok(position_commits(nodes))
}

fn walk_commits(repo: &Repository) -> Result<Vec<CommitNode>, git2::Error> {
    // Build ref labels map: oid -> Vec<RefLabel>
    let mut ref_map: HashMap<String, Vec<RefLabel>> = HashMap::new();

    let head_oid = repo.head().ok()
        .and_then(|h| h.resolve().ok())
        .and_then(|h| h.target())
        .map(|oid| oid.to_string());

    for reference in repo.references()? {
        let reference = reference?;
        let Some(name) = reference.name() else { continue };

        // Peel to commit to handle annotated tags
        let Ok(obj) = reference.peel(git2::ObjectType::Commit) else { continue };
        let oid_str = obj.id().to_string();

        let kind = if name == "HEAD" {
            continue; // HEAD handled separately below
        } else if name.starts_with("refs/tags/") {
            RefKind::Tag
        } else if name.starts_with("refs/remotes/") {
            RefKind::RemoteBranch
        } else {
            RefKind::Branch
        };

        let short_name = reference.shorthand().unwrap_or(name).to_string();
        ref_map.entry(oid_str).or_default().push(RefLabel { name: short_name, kind });
    }

    // Mark HEAD commit
    if let Some(head) = &head_oid {
        ref_map.entry(head.clone()).or_default().push(RefLabel {
            name: "HEAD".to_string(),
            kind: RefKind::Head,
        });
    }

    // Walk all commits reachable from any ref
    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;

    for reference in repo.references()? {
        let reference = reference?;
        if let Some(oid) = reference.target() {
            // push_ref can fail for symbolic refs like HEAD, ignore
            let _ = revwalk.push(oid);
        }
    }

    let mut nodes = Vec::new();

    for oid_result in revwalk {
        let oid: Oid = oid_result?;
        let commit = repo.find_commit(oid)?;

        let oid_str = oid.to_string();
        let parent_oids = commit.parent_ids()
            .map(|p| p.to_string())
            .collect();

        nodes.push(CommitNode {
            refs: ref_map.remove(&oid_str).unwrap_or_default(),
            summary: commit.summary().unwrap_or("").to_string(),
            author: commit.author().name().unwrap_or("").to_string(),
            time: commit.time().seconds(),
            parent_oids,
            oid: oid_str,
        });
    }

    Ok(nodes)
}

// --- Lane assignment ---
//
// Algorithm: maintain a list of "active lanes", one per in-progress branch.
// Each lane tracks which commit oid it's heading toward (its next expected parent).
// For each commit (top to bottom = newest to oldest):
//   1. Find which lane(s) are expecting this commit as a parent.
//   2. The first such lane becomes this commit's lane (merge target).
//   3. Other lanes expecting this commit are merged in (their edges terminate here).
//   4. Advance surviving lanes to point at this commit's first parent.
//   5. If this commit has additional parents (merge), open new lanes for them.
//   6. If no lane was expecting this commit, open a new lane (new branch tip).

fn position_commits(nodes: Vec<CommitNode>) -> PositionedGraph {
    let mut commits: Vec<PositionedCommit> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();

    // active_lanes[i] = Some(oid) means lane i is tracking toward that parent oid
    let mut active_lanes: Vec<Option<String>> = Vec::new();
    let mut max_lane = 0usize;

    // Index nodes by oid for quick lookup (not needed for positioning but useful for edges)
    // We process in revwalk order: newest first

    for (row, node) in nodes.iter().enumerate() {
        // Step 1 & 2: find lanes expecting this commit
        let expecting: Vec<usize> = active_lanes.iter().enumerate()
            .filter_map(|(i, slot)| {
                if slot.as_deref() == Some(&node.oid) { Some(i) } else { None }
            })
            .collect();

        let commit_lane = if let Some(&first) = expecting.first() {
            first
        } else {
            // No lane expected this commit — it's a new branch tip, open a lane
            let lane = first_free_lane(&active_lanes);
            if lane >= active_lanes.len() {
                active_lanes.push(None);
            }
            lane
        };

        max_lane = max_lane.max(commit_lane);

        // Step 3: emit edges from lanes that were tracking toward this commit
        for &lane in &expecting {
            // The edge comes from wherever that lane was last active.
            // We track this by looking at previous commits — simplification:
            // emit edge from (lane, row) downward to parent row when we process parents.
            // For now mark the lane as needing update.
            if lane != commit_lane {
                // Merging lane — will close after this commit
                active_lanes[lane] = None;
            }
        }

        // Step 4 & 5: advance lanes to parents
        let mut parent_iter = node.parent_oids.iter();

        // First parent continues on commit_lane
        if let Some(first_parent) = parent_iter.next() {
            active_lanes[commit_lane] = Some(first_parent.clone());
        } else {
            // Root commit, lane is done
            active_lanes[commit_lane] = None;
        }

        // Additional parents (merge commits) get new lanes
        for extra_parent in parent_iter {
            let lane = first_free_lane(&active_lanes);
            if lane >= active_lanes.len() {
                active_lanes.push(None);
            }
            active_lanes[lane] = Some(extra_parent.clone());
            max_lane = max_lane.max(lane);
        }

        commits.push(PositionedCommit {
            node: node.clone(),
            row,
            lane: commit_lane,
        });
    }

    // Build edges from positioned commits
    // For each commit, find the row of each parent and emit an edge
    let row_by_oid: HashMap<&str, (usize, usize)> = commits.iter()
        .map(|c| (c.node.oid.as_str(), (c.row, c.lane)))
        .collect();

    for commit in &commits {
        for parent_oid in &commit.node.parent_oids {
            if let Some(&(parent_row, parent_lane)) = row_by_oid.get(parent_oid.as_str()) {
                edges.push(GraphEdge {
                    from_row: commit.row,
                    from_lane: commit.lane,
                    to_row: parent_row,
                    to_lane: parent_lane,
                });
            }
        }
    }

    PositionedGraph {
        commits,
        edges,
        lane_count: max_lane + 1,
    }
}

fn first_free_lane(lanes: &[Option<String>]) -> usize {
    lanes.iter().position(|s| s.is_none()).unwrap_or(lanes.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature, Time};
    use std::collections::HashMap;
    use tempfile::TempDir;

    // --- Helpers ---

    fn sig() -> Signature<'static> {
        Signature::new("Test User", "test@example.com", &Time::new(1000, 0)).unwrap()
    }

    /// Create an empty commit with given parents. Returns the OID string.
    fn commit(repo: &Repository, message: &str, parents: &[git2::Oid]) -> git2::Oid {
        let tree = {
            let mut index = repo.index().unwrap();
            let tree_oid = index.write_tree().unwrap();
            repo.find_tree(tree_oid).unwrap()
        };

        let parent_commits: Vec<git2::Commit> = parents.iter()
            .map(|oid| repo.find_commit(*oid).unwrap())
            .collect();
        let parent_refs: Vec<&git2::Commit> = parent_commits.iter().collect();

        repo.commit(None, &sig(), &sig(), message, &tree, &parent_refs).unwrap()
    }

    /// Point a branch ref at an oid.
    fn create_branch(repo: &Repository, name: &str, oid: git2::Oid) {
        let commit = repo.find_commit(oid).unwrap();
        repo.branch(name, &commit, true).unwrap();
    }

    fn graph_from_repo(repo: &Repository) -> PositionedGraph {
        let nodes = walk_commits(repo).unwrap();
        position_commits(nodes)
    }

    fn commit_by_summary<'a>(graph: &'a PositionedGraph, summary: &str) -> &'a PositionedCommit {
        graph.commits.iter()
            .find(|c| c.node.summary == summary)
            .unwrap_or_else(|| panic!("commit '{}' not found in graph", summary))
    }

    // --- Tests ---

    #[test]
    fn test_linear_history_rows_are_sequential() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        let a = commit(&repo, "A", &[]);
        let b = commit(&repo, "B", &[a]);
        let c = commit(&repo, "C", &[b]);
        create_branch(&repo, "main", c);

        let graph = graph_from_repo(&repo);

        assert_eq!(graph.commits.len(), 3);

        // Newest commit should be row 0
        let ca = commit_by_summary(&graph, "C");
        let cb = commit_by_summary(&graph, "B");
        let cc = commit_by_summary(&graph, "A");
        assert!(ca.row < cb.row, "C should be above B");
        assert!(cb.row < cc.row, "B should be above A");
    }

    #[test]
    fn test_linear_history_single_lane() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        let a = commit(&repo, "A", &[]);
        let b = commit(&repo, "B", &[a]);
        create_branch(&repo, "main", b);

        let graph = graph_from_repo(&repo);

        assert!(graph.commits.iter().all(|c| c.lane == 0), "linear history should use one lane");
        assert_eq!(graph.lane_count, 1);
    }

    #[test]
    fn test_branch_uses_second_lane() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        //   B (feature)
        //   |
        // C-+  (main, branches off A)
        //   |
        //   A

        let a = commit(&repo, "A", &[]);
        let b = commit(&repo, "B", &[a]); // feature branch
        let c = commit(&repo, "C", &[a]); // main

        create_branch(&repo, "main", c);
        create_branch(&repo, "feature", b);

        let graph = graph_from_repo(&repo);

        let lanes: Vec<usize> = graph.commits.iter().map(|c| c.lane).collect();
        let max_lane = lanes.iter().max().copied().unwrap();
        assert!(max_lane >= 1, "branching history should use at least 2 lanes");
        assert_eq!(graph.lane_count, max_lane + 1);
    }

    #[test]
    fn test_merge_commit_edges() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        //   M   (merge commit, main)
        //  / \
        // B   C
        //  \ /
        //   A

        let a = commit(&repo, "A", &[]);
        let b = commit(&repo, "B", &[a]);
        let c = commit(&repo, "C", &[a]);
        let m = commit(&repo, "M", &[b, c]);
        create_branch(&repo, "main", m);

        let graph = graph_from_repo(&repo);

        let merge = commit_by_summary(&graph, "M");
        let edges_from_merge: Vec<&GraphEdge> = graph.edges.iter()
            .filter(|e| e.from_row == merge.row)
            .collect();

        assert_eq!(edges_from_merge.len(), 2, "merge commit should have 2 outgoing edges");
    }

    #[test]
    fn test_root_commit_has_no_edges() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        let a = commit(&repo, "A", &[]);
        create_branch(&repo, "main", a);

        let graph = graph_from_repo(&repo);

        assert_eq!(graph.edges.len(), 0, "root commit should produce no edges");
    }

    #[test]
    fn test_edge_count_equals_parent_count() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        let a = commit(&repo, "A", &[]);
        let b = commit(&repo, "B", &[a]);
        let c = commit(&repo, "C", &[a]);
        let m = commit(&repo, "M", &[b, c]);
        create_branch(&repo, "main", m);

        let graph = graph_from_repo(&repo);

        // A(0 parents) + B(1) + C(1) + M(2) = 4 edges total
        assert_eq!(graph.edges.len(), 4);
    }

    #[test]
    fn test_tag_ref_label_kind() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        let a = commit(&repo, "A", &[]);
        create_branch(&repo, "main", a);

        // Create a lightweight tag
        let obj = repo.find_object(a, None).unwrap();
        repo.tag_lightweight("v1.0.0", &obj, false).unwrap();

        let graph = graph_from_repo(&repo);

        let commit_a = commit_by_summary(&graph, "A");
        let tag = commit_a.node.refs.iter().find(|r| r.kind == RefKind::Tag);
        assert!(tag.is_some(), "commit should have a tag ref label");
        assert_eq!(tag.unwrap().name, "v1.0.0");
    }

    #[test]
    fn test_all_commits_present() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        let a = commit(&repo, "A", &[]);
        let b = commit(&repo, "B", &[a]);
        let c = commit(&repo, "C", &[b]);
        create_branch(&repo, "main", c);

        let graph = graph_from_repo(&repo);
        assert_eq!(graph.commits.len(), 3);
    }

    #[test]
    fn test_unreachable_commits_excluded() {
        // Commits only reachable if we push all ref heads — commits with no
        // ref pointing at them (dangling) should not appear.
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        let a = commit(&repo, "A", &[]);
        let _dangling = commit(&repo, "Dangling", &[a]); // no ref points here
        create_branch(&repo, "main", a);

        let graph = graph_from_repo(&repo);

        // Only A should appear; Dangling has no ref
        assert_eq!(graph.commits.len(), 1);
        assert!(commit_by_summary(&graph, "A").row == 0);
    }
}