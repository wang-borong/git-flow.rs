use crate::{config::definition::BranchType, echo::Echo, git::Git, utils::run_hook};

pub fn delete_branch(branch_name: String, branch_type: BranchType, remote: bool, force: bool) {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    // -- validate branches --
    let branches = match git.get_local_branches() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(branches_v) => branches_v,
    };
    if branches.iter().all(|x| x.as_str() != branch_name) {
        Echo::error(format!("target branch {} not found", branch_name));
        return;
    }

    // Protect core/base branches from deletion
    let config = crate::config::read::read_config(None).ok();
    let is_core_branch = {
        let normalized = branch_name
            .strip_prefix("refs/heads/")
            .unwrap_or(&branch_name);
        if let Some(ref cfg) = config {
            let mut core_branches = vec![
                cfg.base_branch
                    .clone()
                    .unwrap_or_else(|| "main".to_string()),
                "main".to_string(),
                "master".to_string(),
                "dev".to_string(),
                "develop".to_string(),
            ];
            for bt in &cfg.branch_types {
                core_branches.push(bt.from.clone());
            }
            core_branches.contains(&normalized.to_string())
        } else {
            normalized == "main"
                || normalized == "master"
                || normalized == "dev"
                || normalized == "develop"
        }
    };

    if is_core_branch {
        Echo::error(format!(
            "safety rule: refusing to delete core branch '{}'",
            branch_name
        ));
        return;
    }

    // -- validate merge status if force is false --
    if !force {
        let mut is_merged = false;

        // Check if merged into source branch
        if let Ok((ahead, _)) = git.get_ahead_behind(&branch_name, &branch_type.from) {
            if ahead == 0 {
                is_merged = true;
            }
        }

        // Check if merged into any target branch (resolving regex patterns)
        if !is_merged {
            for local_b in &branches {
                for target in &branch_type.to {
                    if let Ok(regex) = regex::Regex::new(&target.name) {
                        if regex.is_match(local_b) {
                            if let Ok((ahead, _)) = git.get_ahead_behind(&branch_name, local_b) {
                                if ahead == 0 {
                                    is_merged = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                if is_merged {
                    break;
                }
            }
        }

        if !is_merged {
            Echo::error(format!(
                "safety rule: branch '{}' is not fully merged into '{}' or any target branch. \
                 To force delete, run with the --force flag.",
                branch_name, branch_type.from
            ));
            return;
        }
    }

    // Check if it is the current branch
    if let Ok(curr) = git.current_branch() {
        if curr == branch_name {
            // Switch to source branch first before deleting
            let finish = Echo::progress(format!("switch to branch {}", &branch_type.from));
            match git.switch(&branch_type.from) {
                Err(err) => {
                    finish(false, &err.to_string());
                    return;
                }
                Ok(_) => finish(true, &format!("switch to branch {}", &branch_type.from)),
            }
        }
    }

    // -- run before drop hook --
    if run_hook(branch_type.before_drop.clone(), &branch_name, &branch_type).is_err() {
        return;
    }

    // -- delete local branch --
    let finish = Echo::progress(format!("delete local branch {}", &branch_name));
    match git.del_local_branch(&branch_name) {
        Err(err) => {
            finish(false, &err.to_string());
            return;
        }
        Ok(_) => finish(true, &format!("delete local branch {}", &branch_name)),
    }

    // -- delete remote branch if requested --
    if remote {
        let remote_name = branch_type.remote.clone().unwrap_or_else(|| {
            git.get_remote_repos()
                .ok()
                .and_then(|r| r.first().cloned())
                .unwrap_or_else(|| "origin".to_string())
        });
        let finish = Echo::progress(format!(
            "delete remote branch {}/{}",
            remote_name, &branch_name
        ));
        match git.del_remote_branch(&remote_name, &branch_name) {
            Err(err) => {
                finish(false, &err.to_string());
            }
            Ok(_) => finish(
                true,
                &format!("delete remote branch {}/{}", remote_name, &branch_name),
            ),
        }
    }

    // -- run after drop hook --
    let _ = run_hook(branch_type.after_drop.clone(), &branch_name, &branch_type);
}
