use std::process::Command;

#[derive(Debug)]
pub struct GitInfo {
    pub state: GitState,
    pub branch: String,
    pub changed_files: u32,
    pub modified: u32,
    pub added: u32,
    pub deleted: u32,
    pub untracked: u32,
    /// Commits ahead of upstream. None when no upstream is set.
    pub ahead: Option<u32>,
    /// Commits behind upstream. None when no upstream is set.
    pub behind: Option<u32>,
    /// Total worktrees including the main one.
    pub worktrees: u32,
}

#[derive(Debug)]
pub enum GitState {
    NotRepo,
    Clean,
    Dirty,
}

impl Default for GitInfo {
    fn default() -> Self {
        Self {
            state: GitState::NotRepo,
            branch: String::new(),
            changed_files: 0,
            modified: 0,
            added: 0,
            deleted: 0,
            untracked: 0,
            ahead: None,
            behind: None,
            worktrees: 1,
        }
    }
}

/// Runs `git status --porcelain=v2 --branch --untracked-files=all`.
/// Always returns a valid GitInfo — never propagates errors.
pub fn get_git_info(dir: &str) -> GitInfo {
    if dir.is_empty() {
        return GitInfo::default();
    }

    let output = Command::new("git")
        .args([
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=all",
        ])
        .current_dir(dir)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let mut info = parse_porcelain_v2(&text);
            info.worktrees = get_worktree_count(dir);
            info
        }
        _ => GitInfo::default(),
    }
}

fn get_worktree_count(dir: &str) -> u32 {
    let output = Command::new("git")
        .args(["worktree", "list"])
        .current_dir(dir)
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let count = String::from_utf8_lossy(&o.stdout).lines().count() as u32;
            count.max(1)
        }
        _ => 1,
    }
}

pub(crate) fn parse_porcelain_v2(output: &str) -> GitInfo {
    if output.is_empty() {
        return GitInfo::default();
    }

    let mut branch = String::from("(detached HEAD)");
    let mut modified: u32 = 0;
    let mut added: u32 = 0;
    let mut deleted: u32 = 0;
    let mut untracked: u32 = 0;
    let mut ahead: Option<u32> = None;
    let mut behind: Option<u32> = None;

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            if rest != "(detached)" {
                branch = rest.to_string();
            }
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            // format: "+A -B"
            let mut parts = rest.split_whitespace();
            if let (Some(a), Some(b)) = (parts.next(), parts.next()) {
                ahead = a.trim_start_matches('+').parse().ok();
                behind = b.trim_start_matches('-').parse().ok();
            }
        } else if !line.starts_with('#') && !line.is_empty() {
            if line.starts_with('?') {
                untracked += 1;
            } else {
                // porcelain v2: "1 XY ...", "2 XY ...", "u ..."
                // X = index status (position 2), Y = worktree status (position 3)
                let x = line.chars().nth(2).unwrap_or('.');
                let y = line.chars().nth(3).unwrap_or('.');
                if x == 'A' {
                    added += 1;
                } else if x == 'D' || y == 'D' {
                    deleted += 1;
                } else {
                    modified += 1;
                }
            }
        }
    }

    let changed_files = modified + added + deleted + untracked;
    let state = if changed_files == 0 {
        GitState::Clean
    } else {
        GitState::Dirty
    };

    GitInfo {
        state,
        branch,
        changed_files,
        modified,
        added,
        deleted,
        untracked,
        ahead,
        behind,
        worktrees: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_repo() {
        let output = "# branch.oid abc123\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +0 -0\n";
        let info = parse_porcelain_v2(output);
        assert!(matches!(info.state, GitState::Clean));
        assert_eq!(info.branch, "main");
        assert_eq!(info.changed_files, 0);
        assert_eq!(info.ahead, Some(0));
        assert_eq!(info.behind, Some(0));
    }

    #[test]
    fn parses_dirty_repo() {
        let output = "# branch.oid abc123\n# branch.head feature/x\n# branch.ab +2 -1\n1 .M N... 100644 100644 100644 aaa bbb src/lib.rs\n? untracked.txt\n";
        let info = parse_porcelain_v2(output);
        assert!(matches!(info.state, GitState::Dirty));
        assert_eq!(info.branch, "feature/x");
        assert_eq!(info.changed_files, 2);
        assert_eq!(info.modified, 1);
        assert_eq!(info.untracked, 1);
        assert_eq!(info.ahead, Some(2));
        assert_eq!(info.behind, Some(1));
    }

    #[test]
    fn detached_head_fallback() {
        let output = "# branch.oid abc123\n# branch.head (detached)\n# branch.ab +0 -0\n";
        let info = parse_porcelain_v2(output);
        assert_eq!(info.branch, "(detached HEAD)");
    }

    #[test]
    fn empty_output_returns_not_repo() {
        let info = parse_porcelain_v2("");
        assert!(matches!(info.state, GitState::NotRepo));
    }

    #[test]
    fn no_upstream() {
        // New repo with no upstream set has no branch.ab line
        let output = "# branch.oid abc123\n# branch.head main\n";
        let info = parse_porcelain_v2(output);
        assert!(matches!(info.state, GitState::Clean));
        assert_eq!(info.branch, "main");
        assert_eq!(info.ahead, None);
        assert_eq!(info.behind, None);
    }

    #[test]
    fn ahead_only() {
        let output = "# branch.oid abc123\n# branch.head main\n# branch.ab +3 -0\n";
        let info = parse_porcelain_v2(output);
        assert_eq!(info.ahead, Some(3));
        assert_eq!(info.behind, Some(0));
    }

    #[test]
    fn behind_only() {
        let output = "# branch.oid abc123\n# branch.head main\n# branch.ab +0 -5\n";
        let info = parse_porcelain_v2(output);
        assert_eq!(info.ahead, Some(0));
        assert_eq!(info.behind, Some(5));
    }
}
