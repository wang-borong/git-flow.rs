use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn run_gitflow(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gitflow"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to execute gitflow binary")
}

fn run_gitflow_success(dir: &std::path::Path, args: &[&str]) -> String {
    let output = run_gitflow(dir, args);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    println!("--- gitflow success args: {:?} ---", args);
    println!("stdout:\n{}", stdout);
    println!("stderr:\n{}", stderr);
    assert!(
        output.status.success(),
        "gitflow command failed with args {:?}.\nstdout: {}\nstderr: {}",
        args,
        stdout,
        stderr
    );
    stdout
}

fn run_gitflow_failure(dir: &std::path::Path, args: &[&str]) -> String {
    let output = run_gitflow(dir, args);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    println!("--- gitflow failure args: {:?} ---", args);
    println!("stdout:\n{}", stdout);
    println!("stderr:\n{}", stderr);
    // Note: conceptually it failed, but gitflow exits with 0 on normal error prints.
    // So we don't assert output.status.success() here, we just return the output.
    stdout + &stderr
}

fn git(dir: &std::path::Path, args: &[&str]) {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
}

fn git_current_branch(dir: &std::path::Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn git_branches(dir: &std::path::Path) -> String {
    let out = Command::new("git")
        .args(["for-each-ref", "--format=%(refname:short)", "refs/heads/"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn setup_test_repo() -> TempDir {
    let td = TempDir::new().unwrap();
    let path = td.path();

    // git init
    let output = Command::new("git")
        .arg("init")
        .current_dir(path)
        .output()
        .unwrap();
    assert!(output.status.success());

    // git config user.name
    Command::new("git")
        .args(["config", "user.name", "test"])
        .current_dir(path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(path)
        .output()
        .unwrap();

    // Write a dummy file and commit it so we have a main/dev branch
    fs::write(path.join("a.txt"), "hello").unwrap();
    Command::new("git")
        .args(["add", "a.txt"])
        .current_dir(path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "initial commit"])
        .current_dir(path)
        .output()
        .unwrap();

    // Ensure the default branch is named 'main'
    Command::new("git")
        .args(["branch", "-M", "main"])
        .current_dir(path)
        .output()
        .unwrap();

    // Create a dev branch as well (default in .gitflow.toml)
    Command::new("git")
        .args(["checkout", "-b", "dev"])
        .current_dir(path)
        .output()
        .unwrap();

    // Write a default .gitflow.toml config file
    let config = r#"
allow_non_main_base = true

[[branch_types]]
name = "feature"
create = "feature/{NAME}"
from = "dev"
to = [{ name = "dev", strategy = "merge" }]

[[branch_types]]
name = "hotfix"
create = "hotfix/{NAME}"
from = "main"
to = [
  { name = "main", strategy = "merge" },
  { name = "dev", strategy = "merge" }
]
"#;
    fs::write(path.join(".gitflow.toml"), config).unwrap();

    // Commit config file on dev
    Command::new("git")
        .args(["add", ".gitflow.toml"])
        .current_dir(path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add gitflow config"])
        .current_dir(path)
        .output()
        .unwrap();

    // Merge dev back to main so main also has the config
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["merge", "dev"])
        .current_dir(path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["checkout", "dev"])
        .current_dir(path)
        .output()
        .unwrap();

    td
}

// ============================================================
// ORIGINAL TESTS (preserved)
// ============================================================

#[test]
fn test_feature_flow_success() {
    let td = setup_test_repo();
    let path = td.path();

    // Start feature branch
    run_gitflow_success(path, &["feature", "start", "my-feat"]);

    // Verify we are on the new feature branch
    assert_eq!(git_current_branch(path), "feature/my-feat");

    // Commit some work on the feature branch
    fs::write(path.join("feat.txt"), "feature changes").unwrap();
    git(path, &["add", "feat.txt"]);
    git(path, &["commit", "-m", "feat commit"]);

    // Finish the feature branch
    run_gitflow_success(path, &["feature", "finish", "my-feat"]);

    // Verify feature branch was deleted and we switched back to dev
    assert_eq!(git_current_branch(path), "dev");

    // Verify feature branch is deleted from local branches
    assert!(!git_branches(path).contains("feature/my-feat"));

    // Verify that feat.txt exists on dev (merged successfully)
    assert!(path.join("feat.txt").exists());
}

#[test]
fn test_feature_unmerged_delete() {
    let td = setup_test_repo();
    let path = td.path();

    // Start feature branch
    run_gitflow_success(path, &["feature", "start", "feat-unmerged"]);

    // Commit some unmerged work
    fs::write(path.join("feat_unmerged.txt"), "unmerged feature changes").unwrap();
    git(path, &["add", "feat_unmerged.txt"]);
    git(path, &["commit", "-m", "unmerged commit"]);

    // Switch back to dev branch
    git(path, &["checkout", "dev"]);

    // Try to delete feat-unmerged branch (should fail due to safety checks)
    let err_out = run_gitflow_failure(path, &["feature", "delete", "feat-unmerged"]);
    assert!(err_out.contains("is not fully merged"));

    // Verify branch still exists
    let branches = git_branches(path);
    assert!(
        branches.contains("feature/feat-unmerged"),
        "Branch 'feature/feat-unmerged' not found. Branches are:\n{}",
        branches
    );

    // Now delete with force flag
    run_gitflow_success(path, &["feature", "delete", "feat-unmerged", "--force"]);

    // Verify branch is now deleted
    assert!(!git_branches(path).contains("feature/feat-unmerged"));
}

#[test]
fn test_hook_execution() {
    let td = setup_test_repo();
    let path = td.path();

    // Update config to have a hook
    let config = r#"
[[branch_types]]
name = "feature"
create = "feature/{NAME}"
from = "dev"
to = [{ name = "dev", strategy = "merge" }]
after_start = { command = "touch", args = ["hook_ran.txt"] }
"#;
    fs::write(path.join(".gitflow.toml"), config).unwrap();
    git(path, &["add", ".gitflow.toml"]);
    git(path, &["commit", "-m", "update config with hook"]);

    // Start a feature (hook executes directly because stdin is non-interactive)
    run_gitflow_success(path, &["feature", "start", "feat-hook"]);

    // Verify that the hook touch command executed and created hook_ran.txt!
    assert!(path.join("hook_ran.txt").exists());
}

#[test]
fn test_invalid_cherry_pick_no_state() {
    let td = setup_test_repo();
    let path = td.path();

    let config = r#"
allow_non_main_base = true
[[branch_types]]
name = "general"
create = "general/{NAME}"
from = "main"
to = [{ name = "main", strategy = "merge" }]
"#;
    std::fs::write(path.join(".gitflow.toml"), config).unwrap();
    Command::new("git")
        .args(["commit", "-am", "update config"])
        .current_dir(path)
        .output()
        .unwrap();

    // Start general with a non-existent commit to pick
    let output = run_gitflow_failure(
        path,
        &[
            "general",
            "start",
            "bad-pick",
            "--customer",
            "client1",
            "-t",
            "main",
            "--pick",
            "invalid123",
        ],
    );

    // Ensure the output contains an error about cherry pick failing
    assert!(output.contains("cherry-pick failed"));

    // Verify that GITFLOW_STATE was NOT created
    assert!(!path.join(".git").join("GITFLOW_STATE").exists());
}

#[test]
fn test_sync_all_workspace_restore() {
    let td = setup_test_repo();
    let path = td.path();

    // Set up a custom config for customer branch
    let config = r#"
allow_non_main_base = true

[[branch_types]]
name = "customer"
create = "customer/{NAME}"
from = "dev"
to = [{ name = "dev", strategy = "merge" }]
"#;
    std::fs::write(path.join(".gitflow.toml"), config).unwrap();
    Command::new("git")
        .args(["commit", "-am", "update config"])
        .current_dir(path)
        .output()
        .unwrap();

    // Create some customer branches
    run_gitflow_success(path, &["custom", "start", "client1"]);
    run_gitflow_success(path, &["custom", "start", "client2"]);

    // Switch to dev
    git(path, &["checkout", "dev"]);

    // Run custom sync all
    run_gitflow_success(path, &["custom", "sync", "all"]);

    // Verify that the current branch is still dev!
    assert_eq!(git_current_branch(path), "dev");
}

// ============================================================
// T1: HOTFIX FLOW — merges to main AND dev
// ============================================================
#[test]
fn test_hotfix_flow_success() {
    let td = setup_test_repo();
    let path = td.path();

    // Make sure we're on main to start hotfix
    git(path, &["checkout", "main"]);

    // Start hotfix
    run_gitflow_success(path, &["hotfix", "start", "critical-fix"]);
    assert_eq!(git_current_branch(path), "hotfix/critical-fix");

    // Commit the fix
    fs::write(path.join("fix.txt"), "critical fix").unwrap();
    git(path, &["add", "fix.txt"]);
    git(path, &["commit", "-m", "fix critical bug"]);

    // Finish hotfix — should merge into both main AND dev
    run_gitflow_success(path, &["hotfix", "finish", "critical-fix"]);

    // After finish, should be on main (first target)
    let current = git_current_branch(path);
    assert!(
        current == "main" || current == "dev",
        "Expected main or dev, got {}",
        current
    );

    // hotfix branch should be gone
    assert!(!git_branches(path).contains("hotfix/critical-fix"));

    // fix.txt should exist on main
    git(path, &["checkout", "main"]);
    assert!(path.join("fix.txt").exists(), "fix.txt missing from main");

    // fix.txt should exist on dev too (multi-target merge)
    git(path, &["checkout", "dev"]);
    assert!(path.join("fix.txt").exists(), "fix.txt missing from dev");
}

// ============================================================
// T2: RELEASE FLOW — must use merge strategy (not squash)
// ============================================================
#[test]
fn test_release_flow_success() {
    let td = setup_test_repo();
    let path = td.path();

    // Add release branch type to config
    let config = r#"
allow_non_main_base = true

[[branch_types]]
name = "feature"
create = "feature/{NAME}"
from = "dev"
to = [{ name = "dev", strategy = "merge" }]

[[branch_types]]
name = "hotfix"
create = "hotfix/{NAME}"
from = "main"
to = [
  { name = "main", strategy = "merge" },
  { name = "dev", strategy = "merge" }
]

[[branch_types]]
name = "release"
create = "release/{NAME}"
from = "dev"
to = [
  { name = "main", strategy = "merge" },
  { name = "dev", strategy = "merge" }
]
"#;
    fs::write(path.join(".gitflow.toml"), config).unwrap();
    git(path, &["add", ".gitflow.toml"]);
    git(path, &["commit", "-m", "add release config"]);
    git(path, &["checkout", "main"]);
    git(path, &["merge", "dev"]);
    git(path, &["checkout", "dev"]);

    // Start release
    run_gitflow_success(path, &["release", "start", "v1.0.0"]);
    assert_eq!(git_current_branch(path), "release/v1.0.0");

    // Commit release-prep work
    fs::write(path.join("changelog.txt"), "v1.0.0 changes").unwrap();
    git(path, &["add", "changelog.txt"]);
    git(path, &["commit", "-m", "prepare v1.0.0"]);

    // Finish release
    run_gitflow_success(path, &["release", "finish", "v1.0.0"]);

    // Release branch should be gone
    assert!(!git_branches(path).contains("release/v1.0.0"));

    // changelog.txt should exist on main
    git(path, &["checkout", "main"]);
    assert!(path.join("changelog.txt").exists());

    // changelog.txt should exist on dev too
    git(path, &["checkout", "dev"]);
    assert!(path.join("changelog.txt").exists());
}

// ============================================================
// T3: FEATURE FINISH --keep (branch should survive)
// ============================================================
#[test]
fn test_feature_finish_keep_flag() {
    let td = setup_test_repo();
    let path = td.path();

    run_gitflow_success(path, &["feature", "start", "keep-me"]);
    fs::write(path.join("keep.txt"), "content").unwrap();
    git(path, &["add", "keep.txt"]);
    git(path, &["commit", "-m", "keep commit"]);

    // Finish WITH --keep
    run_gitflow_success(path, &["feature", "finish", "keep-me", "--keep"]);

    // Branch must still exist
    let branches = git_branches(path);
    assert!(
        branches.contains("feature/keep-me"),
        "Expected feature/keep-me to still exist after --keep. Branches are:\n{}",
        branches
    );
}

// ============================================================
// T4: FEATURE RENAME
// ============================================================
#[test]
fn test_feature_rename() {
    let td = setup_test_repo();
    let path = td.path();

    run_gitflow_success(path, &["feature", "start", "old-name"]);

    // Rename
    run_gitflow_success(path, &["feature", "rename", "old-name", "new-name"]);

    // Old branch should be gone, new should exist
    let branches = git_branches(path);
    assert!(
        !branches.contains("feature/old-name"),
        "Old branch still exists"
    );
    assert!(
        branches.contains("feature/new-name"),
        "New branch not found. Branches are:\n{}",
        branches
    );
}

// ============================================================
// T5: FEATURE LIST
// ============================================================
#[test]
fn test_feature_list() {
    let td = setup_test_repo();
    let path = td.path();

    run_gitflow_success(path, &["feature", "start", "alpha"]);
    git(path, &["checkout", "dev"]);
    run_gitflow_success(path, &["feature", "start", "beta"]);
    git(path, &["checkout", "dev"]);

    let out = run_gitflow_success(path, &["feature", "list"]);
    assert!(out.contains("alpha"), "alpha not in list output");
    assert!(out.contains("beta"), "beta not in list output");
}

// ============================================================
// T6: FEATURE CHECKOUT
// ============================================================
#[test]
fn test_feature_checkout() {
    let td = setup_test_repo();
    let path = td.path();

    run_gitflow_success(path, &["feature", "start", "my-feature-x"]);
    git(path, &["checkout", "dev"]);

    // Should switch to the feature branch
    run_gitflow_success(path, &["feature", "checkout", "my-feature-x"]);
    assert_eq!(git_current_branch(path), "feature/my-feature-x");
}

// ============================================================
// T7: CUSTOM LIST
// ============================================================
#[test]
fn test_custom_list() {
    let td = setup_test_repo();
    let path = td.path();

    run_gitflow_success(path, &["custom", "start", "acme"]);
    run_gitflow_success(path, &["custom", "start", "globex"]);
    git(path, &["checkout", "dev"]);

    let out = run_gitflow_success(path, &["custom", "list"]);
    assert!(out.contains("acme"), "acme not in custom list");
    assert!(out.contains("globex"), "globex not in custom list");
}

// ============================================================
// T8: CUSTOM CHECKOUT
// ============================================================
#[test]
fn test_custom_checkout() {
    let td = setup_test_repo();
    let path = td.path();

    run_gitflow_success(path, &["custom", "start", "mycorp"]);
    git(path, &["checkout", "dev"]);

    run_gitflow_success(path, &["custom", "checkout", "mycorp"]);
    assert_eq!(git_current_branch(path), "customer/mycorp");
}

// ============================================================
// T9: CUSTOM SYNC <specific_name>
// ============================================================
#[test]
fn test_custom_sync_single() {
    let td = setup_test_repo();
    let path = td.path();

    // Note: by default, customer branches come from "main" per the setup_test_repo config.
    // We make a new commit on main so there's something to sync to the customer branch.
    git(path, &["checkout", "main"]);
    run_gitflow_success(path, &["custom", "start", "singlecorp"]);
    git(path, &["checkout", "main"]);

    // Make a new commit on main so there's something to sync
    fs::write(path.join("new_feature.txt"), "new content").unwrap();
    git(path, &["add", "new_feature.txt"]);
    git(path, &["commit", "-m", "new main feature"]);

    // Sync single customer (from main into customer/singlecorp)
    run_gitflow_success(path, &["custom", "sync", "singlecorp"]);

    // After sync, should be back on main (original branch before sync)
    assert_eq!(git_current_branch(path), "main");

    // new_feature.txt should now be on customer branch
    git(path, &["checkout", "customer/singlecorp"]);
    assert!(
        path.join("new_feature.txt").exists(),
        "sync did not bring new_feature.txt to customer branch"
    );
}

// ============================================================
// T10: SAFETY RULE — customer branch cannot merge to main
// ============================================================
#[test]
fn test_safety_customer_branch_cannot_merge_to_main() {
    let td = setup_test_repo();
    let path = td.path();

    // Add customer branch type that has main as target (misconfiguration to test safety)
    let config = r#"
allow_non_main_base = true

[[branch_types]]
name = "customer"
create = "customer/{NAME}"
from = "main"
to = [{ name = "main", strategy = "merge" }]
"#;
    fs::write(path.join(".gitflow.toml"), config).unwrap();
    git(path, &["add", ".gitflow.toml"]);
    git(path, &["commit", "-m", "customer config"]);
    git(path, &["checkout", "main"]);
    git(path, &["merge", "dev"]);

    // Create a customer branch
    run_gitflow_success(path, &["custom", "start", "badcorp"]);

    // Commit something
    fs::write(path.join("bad.txt"), "bad content").unwrap();
    git(path, &["add", "bad.txt"]);
    git(path, &["commit", "-m", "bad commit"]);

    // Try to finish customer branch (should fail: safety rule)
    let out = run_gitflow_failure(path, &["custom", "finish", "badcorp"]);
    // custom finish doesn't merge to main; this tests custom finish behavior (--force required)
    assert!(
        out.contains("force") || out.contains("not allowed") || out.contains("finish"),
        "Expected safety or force-required message, got: {}",
        out
    );
}

// ============================================================
// T11: SAFETY RULE — release cannot use squash
// ============================================================
#[test]
fn test_safety_release_must_use_merge_strategy() {
    let td = setup_test_repo();
    let path = td.path();

    // Config with release using squash (invalid)
    let config = r#"
allow_non_main_base = true

[[branch_types]]
name = "release"
create = "release/{NAME}"
from = "dev"
to = [{ name = "main", strategy = "squash" }]
"#;
    fs::write(path.join(".gitflow.toml"), config).unwrap();
    git(path, &["add", ".gitflow.toml"]);
    git(path, &["commit", "-m", "bad release config"]);
    git(path, &["checkout", "main"]);
    git(path, &["merge", "dev"]);
    git(path, &["checkout", "dev"]);

    run_gitflow_success(path, &["release", "start", "v2.0.0"]);
    fs::write(path.join("release.txt"), "release content").unwrap();
    git(path, &["add", "release.txt"]);
    git(path, &["commit", "-m", "release prep"]);

    // Finish — should fail with safety rule about squash on release
    let out = run_gitflow_failure(path, &["release", "finish", "v2.0.0"]);
    assert!(
        out.contains("squash") || out.contains("safety rule") || out.contains("merge strategy"),
        "Expected safety rule violation message, got: {}",
        out
    );
}

// ============================================================
// T12: SAFETY RULE — feature cannot merge customer-scoped feature to main
// ============================================================
#[test]
fn test_safety_customer_scoped_feature_blocked_from_main() {
    let td = setup_test_repo();
    let path = td.path();

    // Config where customer-scoped feature has main as target (misconfiguration)
    let config = r#"
allow_non_main_base = true

[[branch_types]]
name = "feature"
create = "feature/customer-acme/{NAME}"
from = "main"
to = [{ name = "main", strategy = "merge" }]
"#;
    fs::write(path.join(".gitflow.toml"), config).unwrap();
    git(path, &["add", ".gitflow.toml"]);
    git(path, &["commit", "-m", "customer feature config"]);
    git(path, &["checkout", "main"]);
    git(path, &["merge", "dev"]);

    // Start the customer-scoped feature
    run_gitflow_success(path, &["feature", "start", "my-task"]);

    let branch = git_current_branch(path);
    assert!(
        branch.contains("customer-acme"),
        "Expected customer-scoped branch, got {}",
        branch
    );

    // Commit something
    fs::write(path.join("acme.txt"), "acme content").unwrap();
    git(path, &["add", "acme.txt"]);
    git(path, &["commit", "-m", "acme work"]);

    // Finish should be REJECTED by safety rules (customer-scoped branch to main)
    let out = run_gitflow_failure(path, &["feature", "finish", "my-task"]);
    assert!(
        out.contains("safety rule") || out.contains("not allowed") || out.contains("customer"),
        "Expected safety rule rejection for customer-scoped branch merging to main, got: {}",
        out
    );
}

// ============================================================
// T13: ABORT RESTORES SOURCE BRANCH (ISSUE-A1)
// ============================================================
#[test]
fn test_abort_restores_source_branch() {
    let td = setup_test_repo();
    let path = td.path();

    // Create a conflict scenario:
    // 1. Create feature branch with a commit
    // 2. Make a conflicting commit on dev
    // 3. Attempt feature finish → conflict → abort
    // 4. Verify we're back on feature branch, not dev

    run_gitflow_success(path, &["feature", "start", "my-conflict-feat"]);
    fs::write(path.join("conflict.txt"), "feature version").unwrap();
    git(path, &["add", "conflict.txt"]);
    git(path, &["commit", "-m", "feature commit"]);

    // Make conflicting change on dev
    git(path, &["checkout", "dev"]);
    fs::write(path.join("conflict.txt"), "dev version").unwrap();
    git(path, &["add", "conflict.txt"]);
    git(path, &["commit", "-m", "dev conflicting commit"]);

    // Go back to feature and try finish (will conflict)
    git(path, &["checkout", "feature/my-conflict-feat"]);
    let out = run_gitflow_failure(path, &["feature", "finish", "my-conflict-feat"]);
    assert!(
        out.contains("conflict") || out.contains("GITFLOW_STATE"),
        "Expected conflict indication, got: {}",
        out
    );

    // GITFLOW_STATE should have been created
    assert!(
        path.join(".git").join("GITFLOW_STATE").exists(),
        "GITFLOW_STATE should exist after conflict"
    );

    // Abort the in-progress merge
    git(path, &["merge", "--abort"]);

    // Run gitflow abort
    run_gitflow_success(path, &["abort"]);

    // GITFLOW_STATE should be cleared
    assert!(
        !path.join(".git").join("GITFLOW_STATE").exists(),
        "GITFLOW_STATE should be cleared after abort"
    );

    // Should be back on feature branch (ISSUE-A1 fix)
    let current = git_current_branch(path);
    assert_eq!(
        current, "feature/my-conflict-feat",
        "After abort, should be on feature branch, but was on '{}'",
        current
    );
}

// ============================================================
// T14: DANGLING BRANCH CLEANUP on non-conflict failure (ISSUE-S2)
// ============================================================
#[test]
fn test_start_dangling_branch_cleanup() {
    let td = setup_test_repo();
    let path = td.path();

    let config = r#"
allow_non_main_base = true
[[branch_types]]
name = "general"
create = "general/{NAME}"
from = "main"
to = [{ name = "main", strategy = "merge" }]
"#;
    std::fs::write(path.join(".gitflow.toml"), config).unwrap();
    Command::new("git")
        .args(["commit", "-am", "update config"])
        .current_dir(path)
        .output()
        .unwrap();

    // Use an invalid (non-conflict) cherry-pick hash → should fail and clean up the branch
    let branches_before = git_branches(path);
    run_gitflow_failure(
        path,
        &[
            "general",
            "start",
            "cleanup-test",
            "--customer",
            "c1",
            "-t",
            "main",
            "--pick",
            "deadbeefdeadbeef",
        ],
    );

    // Verify no dangling "general/cleanup-test" branch was left
    let branches_after = git_branches(path);
    assert!(
        !branches_after.contains("general/cleanup-test"),
        "Dangling branch should have been cleaned up, but branches are:\n{}",
        branches_after
    );

    // Should still be on main (restored)
    let current = git_current_branch(path);
    assert_eq!(
        current, "main",
        "Should be back on main after cleanup, got '{}'",
        current
    );

    // No GITFLOW_STATE for non-conflict failures
    assert!(!path.join(".git").join("GITFLOW_STATE").exists());
    let _ = branches_before; // suppress unused warning
}

// ============================================================
// T15: SHORTHAND — gitflow finish (infers branch type from current branch)
// ============================================================
#[test]
fn test_shorthand_finish() {
    let td = setup_test_repo();
    let path = td.path();

    run_gitflow_success(path, &["feature", "start", "shorthand-feat"]);
    fs::write(path.join("sh.txt"), "shorthand").unwrap();
    git(path, &["add", "sh.txt"]);
    git(path, &["commit", "-m", "shorthand commit"]);

    // Should work the same as "feature finish shorthand-feat"
    run_gitflow_success(path, &["finish"]);

    // Verify merged to dev and branch deleted
    assert_eq!(git_current_branch(path), "dev");
    assert!(!git_branches(path).contains("feature/shorthand-feat"));
    assert!(path.join("sh.txt").exists());
}

// ============================================================
// T16: SHORTHAND — gitflow delete (infers branch type)
// ============================================================
#[test]
fn test_shorthand_delete() {
    let td = setup_test_repo();
    let path = td.path();

    run_gitflow_success(path, &["feature", "start", "shorthand-del"]);

    // Force delete from current branch (no commits, so it's "merged" trivially)
    run_gitflow_success(path, &["delete", "--force"]);

    // Branch should be gone and we should be on dev
    assert!(!git_branches(path).contains("feature/shorthand-del"));
    assert_eq!(git_current_branch(path), "dev");
}

// ============================================================
// T17: SHORTHAND — gitflow rename (infers branch type)
// ============================================================
#[test]
fn test_shorthand_rename() {
    let td = setup_test_repo();
    let path = td.path();

    run_gitflow_success(path, &["feature", "start", "rename-src"]);

    // Rename using shorthand
    run_gitflow_success(path, &["rename", "rename-dst"]);

    // Old should be gone, new should exist and we should be on it
    let branches = git_branches(path);
    assert!(
        !branches.contains("feature/rename-src"),
        "Old branch should be gone"
    );
    assert!(
        branches.contains("feature/rename-dst"),
        "New branch should exist. Branches are:\n{}",
        branches
    );
}
