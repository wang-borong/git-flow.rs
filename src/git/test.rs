use super::*;

#[test]
fn has_git_t() {
    assert_eq!(Git::git_installed(), true);
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
    assert_eq!(result.is_ok(), false);
    let result = git.switch("main");
    assert_eq!(result.is_ok(), true);
}

#[test]
fn merge_t() {
    let git = Git::open().unwrap();
    let result = git.merge("undefined");
    assert_eq!(result.is_ok(), false);
}

#[test]
fn rebase_t() {
    let git = Git::open().unwrap();
    let result = git.rebase("undefined");
    assert_eq!(result.is_ok(), false);
}

#[test]
fn cherry_pick_t() {
    let git = Git::open().unwrap();
    let result = git.cherry_pick(vec!["undefined".to_string()]);
    assert_eq!(result.is_ok(), false);
}

#[test]
fn del_local_branch_t() {
    let git = Git::open().unwrap();
    let result = git.del_local_branch("undefined");
    assert_eq!(result.is_ok(), false);
}

#[test]
fn diff_commits_t() {
    let git = Git::open().unwrap();
    let result = git.diff_commits("main", "test");
    assert_eq!(result.is_ok(), false)
}

#[test]
fn create_local_branch_t() {
    let git = Git::open().unwrap();
    let result = git.create_local_branch("main", "main");
    assert_eq!(result.is_ok(), false)
}

#[test]
fn create_remote_branch_t() {
    let git = Git::open().unwrap();
    let result = git.create_remote_branch("test", "main", "main");
    assert_eq!(result.is_ok(), false)
}

#[test]
fn get_local_branches_t() {
    let git = Git::open().unwrap();
    let result = git.get_local_branches().unwrap();
    assert_eq!(result.iter().find(|x| x.as_str() == "main").is_some(), true);
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
    assert_eq!(repos.iter().any(|x| x == "origin"), true);
    assert_eq!(repos.len(), 1);
}

#[test]
fn del_remote_branch_t() {
    let git = Git::open().unwrap();
    let result = git.del_remote_branch("test", "main");
    assert_eq!(result.is_ok(), false)
}
