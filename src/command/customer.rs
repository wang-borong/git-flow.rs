use crate::{echo::Echo, git::Git};

/// Create a new customer branch from main.
pub fn create_customer(customer_name: &str, main_branch: &str, remote: Option<&str>) {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    let customer_branch = format!("customer/{}", customer_name);

    // -- validate --
    let branches = match git.get_local_branches() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(b) => b,
    };

    if branches.iter().any(|b| b == &customer_branch) {
        Echo::error(format!(
            "customer branch '{}' already exists",
            customer_branch
        ));
        return;
    }

    if branches.iter().all(|b| b != main_branch) {
        Echo::error(format!("main branch '{}' not found", main_branch));
        return;
    }

    // -- create branch from main --
    let finish = Echo::progress(format!(
        "create '{}' from '{}'",
        customer_branch, main_branch
    ));
    match git.create_local_branch(main_branch, &customer_branch) {
        Err(err) => {
            finish(false, &err.to_string());
            return;
        }
        Ok(_) => finish(
            true,
            &format!("create '{}' from '{}'", customer_branch, main_branch),
        ),
    }

    // -- push to remote if specified --
    if let Some(remote_name) = remote {
        let finish = Echo::progress(format!(
            "push {} to {}/{}",
            customer_branch, remote_name, customer_branch
        ));
        match git.push_branch(remote_name, &customer_branch, &customer_branch) {
            Err(err) => {
                finish(false, &err.to_string());
                return;
            }
            Ok(_) => finish(
                true,
                &format!(
                    "push {} to {}/{}",
                    customer_branch, remote_name, customer_branch
                ),
            ),
        }
    }

    Echo::success(format!(
        "customer branch '{}' created. Switch to it with: git switch {}",
        customer_branch, customer_branch
    ));
}

/// Sync main changes into a customer branch.
pub fn sync_customer(
    customer_name: &str,
    main_branch: &str,
    push: bool,
    rebase: bool,
    remote: Option<&str>,
) {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    let customer_branch = format!("customer/{}", customer_name);

    // -- save original branch for workspace restoration --
    let original_branch = git
        .current_branch()
        .unwrap_or_else(|_| main_branch.to_string());

    // -- validate --
    let branches = match git.get_local_branches() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(b) => b,
    };

    if branches.iter().all(|b| b != &customer_branch) {
        Echo::error(format!("customer branch '{}' not found", customer_branch));
        return;
    }

    // -- switch to customer branch --
    let finish = Echo::progress(format!("switch to {}", customer_branch));
    if let Err(err) = git.switch(&customer_branch) {
        finish(false, &err.to_string());
        return;
    }
    finish(true, &format!("switch to {}", customer_branch));

    // -- sync main into customer --
    if rebase {
        let finish = Echo::progress(format!("rebase {} onto {}", customer_branch, main_branch));
        match git.rebase(main_branch) {
            Err(err) => {
                finish(false, &err.to_string());
                // Save state so `gitflow continue` can resume after conflict resolution
                let state = crate::command::state::GitflowState::Sync {
                    customer_branch: customer_branch.clone(),
                    main_branch: main_branch.to_string(),
                    rebase: true,
                    push,
                    remote: remote.map(|s| s.to_string()),
                    original_branch,
                };
                if let Err(save_err) = state.save(&git) {
                    Echo::error(format!("Failed to save gitflow state: {}", save_err));
                } else {
                    Echo::info(
                        "Gitflow state saved. Resolve the conflict and run 'gitflow continue'.",
                    );
                }
                return;
            }
            Ok(_) => finish(
                true,
                &format!("rebase {} onto {}", customer_branch, main_branch),
            ),
        }
    } else {
        let finish = Echo::progress(format!("merge {} into {}", main_branch, customer_branch));
        match git.merge(main_branch, None) {
            Err(err) => {
                finish(false, &err.to_string());
                // Save state so `gitflow continue` can resume after conflict resolution
                let state = crate::command::state::GitflowState::Sync {
                    customer_branch: customer_branch.clone(),
                    main_branch: main_branch.to_string(),
                    rebase: false,
                    push,
                    remote: remote.map(|s| s.to_string()),
                    original_branch,
                };
                if let Err(save_err) = state.save(&git) {
                    Echo::error(format!("Failed to save gitflow state: {}", save_err));
                } else {
                    Echo::info(
                        "Gitflow state saved. Resolve the conflict and run 'gitflow continue'.",
                    );
                }
                return;
            }
            Ok(_) => finish(
                true,
                &format!("merge {} into {}", main_branch, customer_branch),
            ),
        }
    }

    // -- push if requested --
    if push {
        if let Some(remote_name) = remote {
            let finish = Echo::progress(format!(
                "push {} to {}/{}",
                customer_branch, remote_name, customer_branch
            ));
            match git.push_branch(remote_name, &customer_branch, &customer_branch) {
                Err(err) => {
                    finish(false, &err.to_string());
                    // Still restore the original branch even if push fails
                    let _ = git.switch(&original_branch);
                    return;
                }
                Ok(_) => finish(
                    true,
                    &format!(
                        "push {} to {}/{}",
                        customer_branch, remote_name, customer_branch
                    ),
                ),
            }
        }
    }

    // -- restore original branch --
    if original_branch != customer_branch {
        if let Err(err) = git.switch(&original_branch) {
            Echo::error(format!(
                "Failed to restore original branch '{}': {}",
                original_branch, err
            ));
        }
    }

    Echo::success(format!(
        "'{}' synced with '{}'",
        customer_branch, main_branch
    ));
}

/// Sync all customer branches with main.
pub fn sync_all_customers(
    main_branch: &str,
    push: bool,
    rebase: bool,
    remote: Option<&str>,
    customer_prefix: &str,
) {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    let original_branch = git.current_branch().unwrap_or_else(|_| "main".to_string());

    // -- get all customer branches --
    let branches = match git.get_local_branches() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(b) => b,
    };

    let customer_branches: Vec<&str> = branches
        .iter()
        .filter(|b| b.starts_with(customer_prefix))
        .map(|b| b.as_str())
        .collect();

    if customer_branches.is_empty() {
        Echo::info("no customer branches found");
        return;
    }

    println!(
        "\nSyncing {} customer branches with '{}':\n",
        customer_branches.len(),
        main_branch
    );

    let mut success_count = 0;
    let mut fail_count = 0;
    let mut results: Vec<(String, bool, String)> = Vec::new();

    for customer_branch in &customer_branches {
        // -- switch to customer branch --
        if let Err(err) = git.switch(customer_branch) {
            results.push((customer_branch.to_string(), false, err.to_string()));
            fail_count += 1;
            continue;
        }

        // -- sync main --
        if rebase {
            match git.rebase(main_branch) {
                Err(err) => {
                    // Abort the failed rebase
                    let _ = git.rebase_abort();
                    results.push((
                        customer_branch.to_string(),
                        false,
                        format!("rebase conflict: {}", err),
                    ));
                    fail_count += 1;
                }
                Ok(_) => {
                    // -- push if requested --
                    if push {
                        if let Some(remote_name) = remote {
                            // Using force-with-lease might be needed for rebase, but currently push_branch does normal push
                            // So rebase and push in sync_all might fail if branch is already published
                            if let Err(err) =
                                git.push_branch(remote_name, customer_branch, customer_branch)
                            {
                                results.push((
                                    customer_branch.to_string(),
                                    false,
                                    format!("push failed: {}", err),
                                ));
                                fail_count += 1;
                                continue;
                            }
                        }
                    }
                    results.push((
                        customer_branch.to_string(),
                        true,
                        "synced (rebased)".to_string(),
                    ));
                    success_count += 1;
                }
            }
        } else {
            match git.merge(main_branch, None) {
                Err(err) => {
                    // Abort the failed merge
                    let _ = git.merge_abort();
                    results.push((
                        customer_branch.to_string(),
                        false,
                        format!("merge conflict: {}", err),
                    ));
                    fail_count += 1;
                }
                Ok(_) => {
                    // -- push if requested --
                    if push {
                        if let Some(remote_name) = remote {
                            if let Err(err) =
                                git.push_branch(remote_name, customer_branch, customer_branch)
                            {
                                results.push((
                                    customer_branch.to_string(),
                                    false,
                                    format!("push failed: {}", err),
                                ));
                                fail_count += 1;
                                continue;
                            }
                        }
                    }
                    results.push((
                        customer_branch.to_string(),
                        true,
                        "synced (merged)".to_string(),
                    ));
                    success_count += 1;
                }
            }
        }
    }

    // -- print report --
    println!("\n--- Sync Report ---\n");
    for (branch, success, msg) in &results {
        if *success {
            Echo::success(format!("  {} — {}", branch, msg));
        } else {
            Echo::error(format!("  {} — {}", branch, msg));
        }
    }
    println!();
    Echo::info(format!(
        "Total: {} succeeded, {} failed",
        success_count, fail_count
    ));
    if fail_count > 0 {
        println!();
        Echo::info("Hint: To manually resolve failed branches, run:");
        Echo::info("  gitflow custom sync <branch_name> [--rebase]");
    }

    // -- restore original branch --
    if let Err(err) = git.switch(&original_branch) {
        Echo::error(format!(
            "Failed to restore original branch '{}': {}",
            original_branch, err
        ));
    }
}

/// List all customer branches and their status relative to main.
pub fn list_customers(main_branch: &str, customer_prefix: &str) {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    let branches = match git.get_local_branches() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(b) => b,
    };

    let customer_branches: Vec<&str> = branches
        .iter()
        .filter(|b| b.starts_with(customer_prefix))
        .map(|b| b.as_str())
        .collect();

    if customer_branches.is_empty() {
        Echo::info("no customer branches found");
        return;
    }

    println!("\nCustomer branches:\n");
    for branch in &customer_branches {
        // Try to get diff commits to show how far ahead/behind
        match git.diff_commits(branch, main_branch) {
            Ok(ahead) => match git.diff_commits(main_branch, branch) {
                Ok(behind) => {
                    println!(
                        "  {} ({} ahead, {} behind {})",
                        branch,
                        ahead.len(),
                        behind.len(),
                        main_branch
                    );
                }
                Err(_) => {
                    println!("  {}", branch);
                }
            },
            Err(_) => {
                println!("  {}", branch);
            }
        }
    }
    println!();
}
