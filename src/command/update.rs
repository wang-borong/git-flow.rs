use crate::{config::definition::BranchType, echo::Echo, git::Git};

pub fn update_branch(branch_name: String, branch_type: BranchType, force_rebase: bool) {
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
    if branches.iter().all(|x| x.as_str() != branch_name) {
        Echo::error(format!("branch {} is not found", branch_name));
        return;
    }

    // -- fetch remote first --
    let remote = branch_type.remote.clone().unwrap_or_else(|| {
        git.get_remote_repos()
            .ok()
            .and_then(|r| r.first().cloned())
            .unwrap_or_else(|| "origin".to_string())
    });
    if !remote.is_empty() {
        let finish = Echo::progress(format!("fetch remote {}", remote));
        if let Err(err) = git.fetch_remote(&remote) {
            finish(false, &err.to_string());
            // Continue anyway, maybe offline
        } else {
            finish(true, &format!("fetch remote {}", remote));
        }
    }

    // -- switch to the branch --
    let finish = Echo::progress(format!("switch to branch {}", &branch_name));
    if let Err(err) = git.switch(&branch_name) {
        finish(false, &err.to_string());
        return;
    }
    finish(true, &format!("switch to branch {}", &branch_name));

    // -- determine update strategy --
    // We rebase if force_rebase is true. Otherwise we merge.
    if force_rebase {
        let finish = Echo::progress(format!("rebase {} onto {}", branch_name, branch_type.from));
        match git.rebase(&branch_type.from) {
            Err(err) => {
                finish(false, &err.to_string());
                Echo::info("resolve conflicts, then run `gitflow continue`");
            }
            Ok(_) => {
                finish(
                    true,
                    &format!("rebase {} onto {}", branch_name, branch_type.from),
                );
            }
        }
    } else {
        let finish = Echo::progress(format!("merge {} into {}", branch_type.from, branch_name));
        match git.merge(&branch_type.from, None) {
            Err(err) => {
                finish(false, &err.to_string());
                Echo::info("resolve conflicts, then run `gitflow continue`");
            }
            Ok(_) => {
                finish(
                    true,
                    &format!("merge {} into {}", branch_type.from, branch_name),
                );
            }
        }
    }
}
