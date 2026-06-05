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
    assert!(
        output.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

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
    assert!(
        output.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

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
    assert!(
        !file_path.exists(),
        "a.txt should be removed from the working directory after switching to branch1"
    );

    // 5. Verify that git status is clean (the index file on disk was successfully updated)
    let status_output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()
        .unwrap();
    let status_str = String::from_utf8_lossy(&status_output.stdout);
    assert!(
        status_str.trim().is_empty(),
        "git status should be clean, but got:\n{}",
        status_str
    );
}

#[test]
fn switch_and_cherry_pick_conflict_t() {
    let (td, git) = test_repo();
    let path = td.path();

    // 1. Create a.txt with "heelo" and commit on main
    let a_path = path.join("a.txt");
    std::fs::write(&a_path, "heelo\n").unwrap();
    assert!(std::process::Command::new("git")
        .args(["add", "a.txt"])
        .current_dir(path)
        .status()
        .unwrap()
        .success());
    assert!(std::process::Command::new("git")
        .args(["commit", "-m", "add a"])
        .current_dir(path)
        .status()
        .unwrap()
        .success());

    // 2. Create branch customer/ali pointing to main (contains only a.txt)
    {
        let repo = git.repo.borrow();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("customer/ali", &head, false).unwrap();
    }

    // 3. Create b.txt with "worll" and commit on main (main is now ahead and contains b.txt)
    let b_path = path.join("b.txt");
    std::fs::write(&b_path, "worll\n").unwrap();
    assert!(std::process::Command::new("git")
        .args(["add", "b.txt"])
        .current_dir(path)
        .status()
        .unwrap()
        .success());
    assert!(std::process::Command::new("git")
        .args(["commit", "-m", "add b"])
        .current_dir(path)
        .status()
        .unwrap()
        .success());

    // 4. Switch to customer/ali
    git.switch("customer/ali").unwrap();
    // Write index to disk to simulate a clean switch to customer/ali
    {
        let repo = git.repo.borrow();
        let mut index = repo.index().unwrap();
        index.write().unwrap();
    }

    // 5. Modify a.txt to "hello" and commit on customer/ali
    std::fs::write(&a_path, "hello\n").unwrap();
    assert!(std::process::Command::new("git")
        .args(["add", "a.txt"])
        .current_dir(path)
        .status()
        .unwrap()
        .success());

    let commit_output = std::process::Command::new("git")
        .args(["commit", "-m", "change a"])
        .current_dir(path)
        .output()
        .unwrap();
    assert!(commit_output.status.success());

    // Get the commit hash of the last commit
    let rev_output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(path)
        .output()
        .unwrap();
    let commit_hash = String::from_utf8_lossy(&rev_output.stdout)
        .trim()
        .to_string();

    // 6. Create branch pick-hello from main
    {
        let repo = git.repo.borrow();
        let main_branch = repo.find_branch("main", git2::BranchType::Local).unwrap();
        let main_commit = main_branch.get().peel_to_commit().unwrap();
        repo.branch("pick-hello", &main_commit, false).unwrap();
    }

    // 7. Switch to pick-hello
    git.switch("pick-hello").unwrap();

    // 8. Run git status --porcelain, it should be clean
    let status_output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()
        .unwrap();
    let status_str = String::from_utf8_lossy(&status_output.stdout);
    assert!(
        status_str.trim().is_empty(),
        "git status should be clean, but got:\n{}",
        status_str
    );

    // 9. Cherry-pick the commit
    let cherry_status = std::process::Command::new("git")
        .args(["cherry-pick", &commit_hash])
        .current_dir(path)
        .status()
        .unwrap();
    assert!(cherry_status.success(), "cherry-pick should succeed");
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

#[cfg(test)]
mod extra_tests {
    use super::*;

    #[test]
    fn squash_merge_conflict_t() {
        let (td, git) = test_repo();
        let path = td.path();

        // 1. Create a.txt with "hello" and commit on main
        let a_path = path.join("a.txt");
        std::fs::write(&a_path, "hello\n").unwrap();
        assert!(std::process::Command::new("git")
            .args(["add", "a.txt"])
            .current_dir(path)
            .status()
            .unwrap()
            .success());
        assert!(std::process::Command::new("git")
            .args(["commit", "-m", "add a"])
            .current_dir(path)
            .status()
            .unwrap()
            .success());

        // 2. Create feature branch pointing to main
        {
            let repo = git.repo.borrow();
            let head = repo.head().unwrap().peel_to_commit().unwrap();
            repo.branch("feature", &head, false).unwrap();
        }

        // 3. Modify a.txt to "hello world" and commit on main
        std::fs::write(&a_path, "hello world\n").unwrap();
        assert!(std::process::Command::new("git")
            .args(["add", "a.txt"])
            .current_dir(path)
            .status()
            .unwrap()
            .success());
        assert!(std::process::Command::new("git")
            .args(["commit", "-m", "update a on main"])
            .current_dir(path)
            .status()
            .unwrap()
            .success());

        // 4. Switch to feature branch
        git.switch("feature").unwrap();

        // 5. Modify a.txt to "hello features" and commit on feature branch
        std::fs::write(&a_path, "hello features\n").unwrap();
        assert!(std::process::Command::new("git")
            .args(["add", "a.txt"])
            .current_dir(path)
            .status()
            .unwrap()
            .success());
        assert!(std::process::Command::new("git")
            .args(["commit", "-m", "update a on feature"])
            .current_dir(path)
            .status()
            .unwrap()
            .success());

        // 6. Switch back to main branch
        git.switch("main").unwrap();

        // 7. Try to squash merge feature into main. It should fail due to conflict!
        let merge_res = git.squash_merge("feature", None);
        assert!(merge_res.is_err());
        let err_str = merge_res.err().unwrap().to_string();
        assert!(
            err_str.contains("conflict") || err_str.contains("Conflict"),
            "Actual error: {}",
            err_str
        );

        // 8. Verify that conflict markers exist in the file!
        let content = std::fs::read_to_string(&a_path).unwrap();
        assert!(content.contains("<<<<<<<"));
        assert!(content.contains(">>>>>>>"));
    }
}
