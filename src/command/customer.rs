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

/// Sync main changes into a customer branch (merge strategy).
pub fn sync_customer(customer_name: &str, main_branch: &str, push: bool, remote: Option<&str>) {
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

    // -- merge main into customer --
    let finish = Echo::progress(format!("merge {} into {}", main_branch, customer_branch));
    match git.merge(main_branch) {
        Err(err) => {
            finish(false, &err.to_string());
            Echo::info("resolve conflicts, then run `git-flow continue`");
            return;
        }
        Ok(_) => finish(
            true,
            &format!("merge {} into {}", main_branch, customer_branch),
        ),
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

    Echo::success(format!(
        "'{}' synced with '{}'",
        customer_branch, main_branch
    ));
}

/// Sync all customer branches with main.
pub fn sync_all_customers(
    main_branch: &str,
    push: bool,
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

        // -- merge main --
        match git.merge(main_branch) {
            Err(err) => {
                // Abort the failed merge
                let _ = git.merge_abort();
                results.push((
                    customer_branch.to_string(),
                    false,
                    format!("conflict: {}", err),
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
                results.push((customer_branch.to_string(), true, "synced".to_string()));
                success_count += 1;
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
