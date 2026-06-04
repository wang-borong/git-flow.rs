use crate::{config::definition::BranchType, echo::Echo, git::Git, utils::run_hook};

pub fn delete_branch(branch_name: String, branch_type: BranchType, remote: bool, _force: bool) {
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
