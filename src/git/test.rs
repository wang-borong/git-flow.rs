use super::*;

#[test]
fn has_git_t() {
    assert!(Git::git_installed());
}

#[test]
fn open_repo_t() {
    let git = Git::open();
    assert!(git.is_ok());
}

#[test]
fn switch_t() {
    let git = Git::open().unwrap();
    let result = git.switch("undefined");
    assert!(result.is_err());
    let result = git.switch("main");
    assert!(result.is_ok());
}

#[test]
fn merge_t() {
    let git = Git::open().unwrap();
    let result = git.merge("undefined");
    assert!(result.is_err());
}

#[test]
fn rebase_t() {
    let git = Git::open().unwrap();
    let result = git.rebase("undefined");
    assert!(result.is_err());
}

#[test]
fn cherry_pick_t() {
    let git = Git::open().unwrap();
    let result = git.cherry_pick(vec!["undefined".to_string()]);
    assert!(result.is_err());
}

#[test]
fn del_local_branch_t() {
    let git = Git::open().unwrap();
    let result = git.del_local_branch("undefined");
    assert!(result.is_err());
}

#[test]
fn diff_commits_t() {
    let git = Git::open().unwrap();
    let result = git.diff_commits("main", "test");
    assert!(result.is_err());
}

#[test]
fn create_local_branch_t() {
    let git = Git::open().unwrap();
    let result = git.create_local_branch("main", "main");
    assert!(result.is_err());
}

#[test]
fn create_remote_branch_t() {
    let git = Git::open().unwrap();
    let result = git.create_remote_branch("test", "main", "main");
    assert!(result.is_err());
}

#[test]
fn get_local_branches_t() {
    let git = Git::open().unwrap();
    let result = git.get_local_branches().unwrap();
    assert!(result.iter().any(|x| x.as_str() == "main"));
}

#[test]
fn query_remote_branches_t() {
    let git = Git::open().unwrap();
    git.get_remote_branches("origin").unwrap();
}

#[test]
fn diff_logs_t() {
    let git = Git::open().unwrap();
    git.diff_logs("main", "main").unwrap();
}

#[test]
fn fetch_remote_branches_t() {
    let git = Git::open().unwrap();
    git.fetch_remote_data().unwrap();
}

#[test]
fn get_remote_repos() {
    let git = Git::open().unwrap();
    let repos = git.get_remote_repos().unwrap();
    assert!(repos.iter().any(|x| x == "origin"));
    assert_eq!(repos.len(), 1);
}

#[test]
fn del_remote_branch_t() {
    let git = Git::open().unwrap();
    let result = git.del_remote_branch("test", "main");
    assert!(result.is_err());
}
