use crate::{config::definition::BranchType, echo::Echo, git::Git};

pub fn rename_branch(old_name: String, new_name: String, _branch_type: BranchType) {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    // -- validate branch exists --
    let branches = match git.get_local_branches() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(branches_v) => branches_v,
    };
    let norm_old = old_name.strip_prefix("refs/heads/").unwrap_or(&old_name);
    let norm_new = new_name.strip_prefix("refs/heads/").unwrap_or(&new_name);

    if branches.iter().all(|x| x.as_str() != norm_old) {
        Echo::error(format!("branch {} not found", old_name));
        return;
    }

    // Protect core/base branches from rename operations
    let config = crate::config::read::read_config(None).ok();
    let (is_old_core, is_new_core) = {
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
            (
                core_branches.contains(&norm_old.to_string()),
                core_branches.contains(&norm_new.to_string()),
            )
        } else {
            let old_core = norm_old == "main"
                || norm_old == "master"
                || norm_old == "dev"
                || norm_old == "develop";
            let new_core = norm_new == "main"
                || norm_new == "master"
                || norm_new == "dev"
                || norm_new == "develop";
            (old_core, new_core)
        }
    };

    if is_old_core {
        Echo::error(format!(
            "safety rule: refusing to rename core branch '{}'",
            old_name
        ));
        return;
    }

    if is_new_core {
        Echo::error(format!(
            "safety rule: refusing to rename branch to core branch name '{}'",
            new_name
        ));
        return;
    }

    if branches.iter().any(|x| x.as_str() == norm_new) {
        Echo::error(format!("branch {} already exists", new_name));
        return;
    }

    let finish = Echo::progress(format!("rename branch {} to {}", norm_old, norm_new));
    match git.rename_branch(norm_old, norm_new) {
        Err(err) => {
            finish(false, &err.to_string());
        }
        Ok(_) => {
            finish(true, &format!("rename branch {} to {}", norm_old, norm_new));
        }
    }
}
