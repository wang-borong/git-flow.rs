use super::*;
use git2::Repository;
use tempfile::TempDir;

/// Create a temporary git repo with an initial commit on `main`.
fn test_repo() -> (TempDir, Git) {
    let td = TempDir::new().unwrap();
    let path = td.path();

    // Use git CLI for reliable repo setup
    let output = std::process::Command::new("git")
        .arg("init")
        .current_dir(path)
        .output()
        .unwrap();
    assert!(output.status.success(), "git init failed: {}", String::from_utf8_lossy(&output.stderr));

    std::process::Command::new("git")
        .args(["config", "user.name", "test"])
        .current_dir(path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(path)
        .output()
        .unwrap();
    
    // Config default initial branch if needed, but since we commit first:
    let output = std::process::Command::new("git")
        .args(["commit", "--allow-empty", "-m", "init"])
        .current_dir(path)
        .output()
        .unwrap();
    assert!(output.status.success(), "git commit failed: {}", String::from_utf8_lossy(&output.stderr));

    // Rename active branch to main for consistency
    let _ = std::process::Command::new("git")
        .args(["branch", "-m", "main"])
        .current_dir(path)
        .output();

    let repo = Repository::open(path).unwrap();
    let git = Git::from_repo(repo);
    (td, git)
}

// ---- basic ----

#[test]
fn has_git_t() {
    let (_td, _git) = test_repo();
    assert!(Git::git_installed());
}

#[test]
fn open_repo_t() {
    let (_td, git) = test_repo();
    // Should have a valid current branch (main or master)
    assert!(git.current_branch().is_ok());
}

// ---- switch ----

#[test]
fn switch_t() {
    let (_td, git) = test_repo();
    let result = git.switch("undefined");
    assert!(result.is_err());

    let current = git.current_branch().unwrap();
    let result = git.switch(&current);
    assert!(result.is_ok());
}

#[test]
fn switch_ahead_commits_t() {
    let (td, git) = test_repo();
    let path = td.path();

    // 1. Create a branch "branch1" pointing to the current commit (init)
    {
        let repo = git.repo.borrow();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("branch1", &head, false).unwrap();
    }

    // 2. Add and commit a file on main
    let file_path = path.join("a.txt");
    std::fs::write(&file_path, "content").unwrap();

    let output = std::process::Command::new("git")
        .args(["add", "a.txt"])
        .current_dir(path)
        .output()
        .unwrap();
    assert!(output.status.success());

    let output = std::process::Command::new("git")
        .args(["commit", "-m", "add a.txt"])
        .current_dir(path)
        .output()
        .unwrap();
    assert!(output.status.success());

    // Ensure file exists on main
    assert!(file_path.exists());

    // 3. Switch to branch1
    git.switch("branch1").unwrap();

    // 4. Verify that the file a.txt is removed from the working directory
    assert!(!file_path.exists(), "a.txt should be removed from the working directory after switching to branch1");
}

// ---- merge / rebase / cherry-pick ----

#[test]
fn merge_t() {
    let (_td, git) = test_repo();
    let result = git.merge("undefined", None);
    assert!(result.is_err());
}

#[test]
fn rebase_t() {
    let (_td, git) = test_repo();
    let result = git.rebase("undefined");
    assert!(result.is_err());
}

#[test]
fn cherry_pick_t() {
    let (_td, git) = test_repo();
    let result = git.cherry_pick(vec!["undefined".to_string()]);
    assert!(result.is_err());
}

// ---- branch CRUD ----

#[test]
fn del_local_branch_t() {
    let (_td, git) = test_repo();
    let result = git.del_local_branch("__nonexistent_branch__");
    assert!(result.is_err());
}

#[test]
fn create_local_branch_t() {
    let (_td, git) = test_repo();
    let branches = git.get_local_branches().unwrap();
    assert!(!branches.is_empty());
    // Creating a branch with the same name as an existing one should fail
    let result = git.create_local_branch(&branches[0], &branches[0]);
    assert!(result.is_err());
}

#[test]
fn create_remote_branch_t() {
    let (_td, git) = test_repo();
    let result = git.create_remote_branch("origin", "main", "main");
    assert!(result.is_err());
}

// ---- branch listing ----

#[test]
fn get_local_branches_t() {
    let (_td, git) = test_repo();
    let result = git.get_local_branches().unwrap();
    assert!(!result.is_empty());
}

#[test]
fn query_remote_branches_t() {
    let (_td, git) = test_repo();
    // No remotes in test repo — just returns empty list
    let result = git.get_remote_branches("origin").unwrap();
    assert!(result.is_empty());
}

// ---- diff ----

#[test]
fn diff_commits_t() {
    let (_td, git) = test_repo();
    // Create a second branch so there's something to diff
    {
        let repo = git.repo.borrow();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature", &head, false).unwrap();
    }

    let result = git.diff_commits("feature", &git.current_branch().unwrap());
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn diff_logs_t() {
    let (_td, git) = test_repo();
    let current = git.current_branch().unwrap();
    git.diff_logs(&current, &current).unwrap();
}

// ---- remote ----

#[test]
fn get_remote_repos() {
    let (_td, git) = test_repo();
    // No remotes in test repo
    let repos = git.get_remote_repos().unwrap();
    assert!(repos.is_empty());
}

#[test]
fn del_remote_branch_t() {
    let (_td, git) = test_repo();
    let result = git.del_remote_branch("origin", "main");
    assert!(result.is_err());
}

// ---- fetch (ignored — needs network) ----

#[test]
#[ignore]
fn fetch_remote_branches_t() {
    let (_td, git) = test_repo();
    git.fetch_remote_data().unwrap();
}
