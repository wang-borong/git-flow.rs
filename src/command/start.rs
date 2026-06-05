use crate::{config::definition::BranchType, echo::Echo, git::Git, utils::run_hook};

pub fn start_task(
    branch_name: String,
    branch_type: BranchType,
    fetch: bool,
    pick_commits: Option<Vec<String>>,
) {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    // -- fetch source branch if requested --
    if fetch {
        let remote = branch_type.remote.clone().unwrap_or_else(|| {
            git.get_remote_repos()
                .ok()
                .and_then(|r| r.first().cloned())
                .unwrap_or_default()
        });
        if !remote.is_empty() {
            let finish = Echo::progress(format!("fetch remote {}", remote));
            match git.fetch_remote(&remote) {
                Err(err) => {
                    finish(false, &err.to_string());
                    return;
                }
                Ok(_) => finish(true, &format!("fetch remote {}", remote)),
            }
        }
    }

    // -- validate branches --
    let branches = match git.get_local_branches() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(branches_v) => branches_v,
    };
    if branches.iter().all(|x| x.as_str() != branch_type.from) {
        Echo::error(format!("source branch {} is not found", branch_type.from));
        return;
    }
    if branches.iter().any(|x| x.as_str() == branch_name) {
        Echo::error(format!("branch {} already exists", branch_name));
        return;
    }

    // -- run before start hook --
    if run_hook(branch_type.before_start.clone(), &branch_name, &branch_type).is_err() {
        return;
    }

    // -- create new branch --
    let finish = Echo::progress(format!("create new branch {}", &branch_name));
    match git.create_local_branch(&branch_type.from, &branch_name) {
        Err(err) => {
            finish(false, &err.to_string());
            return;
        }
        Ok(_) => finish(true, &format!("create new branch {}", &branch_name)),
    }

    // -- switch to new branch --
    let finish = Echo::progress(format!("switch to new branch {}", &branch_name));
    match git.switch(&branch_name) {
        Err(err) => {
            finish(false, &err.to_string());
            return;
        }
        Ok(_) => finish(true, &format!("switch to new branch {}", &branch_name)),
    }

    // -- cherry pick commits if specified --
    if let Some(commits) = pick_commits {
        let msg = format!("cherry-pick commits {:?}", commits);
        let finish = Echo::progress(&msg);
        match git.cherry_pick(commits) {
            Err(err) => {
                finish(false, &err.to_string());

                // ISSUE-S3: Use case-insensitive conflict detection
                let err_str = err.to_string().to_lowercase();
                if err_str.contains("conflict") {
                    let state = crate::command::state::GitflowState::Start {
                        branch_name: branch_name.clone(),
                        branch_type: branch_type.clone(),
                    };
                    if let Err(save_err) = state.save(&git) {
                        Echo::error(format!("Failed to save gitflow state: {}", save_err));
                    } else {
                        Echo::info(
                            "Gitflow state saved. Resolve the conflict and run 'gitflow continue'.",
                        );
                    }
                } else {
                    // ISSUE-S2: Non-conflict failure — clean up dangling branch
                    // Switch back to source branch and delete the new (empty) branch
                    let _ = git.switch(&branch_type.from);
                    let del_finish =
                        Echo::progress(format!("cleanup dangling branch {}", branch_name));
                    match git.del_local_branch(&branch_name) {
                        Ok(_) => {
                            del_finish(true, &format!("deleted dangling branch {}", branch_name))
                        }
                        Err(del_err) => del_finish(
                            false,
                            &format!("could not delete {}: {}", branch_name, del_err),
                        ),
                    }
                }
                return;
            }
            Ok(_) => finish(true, &msg),
        }
    }

    // -- run after start hook --
    let _ = run_hook(branch_type.after_start.clone(), &branch_name, &branch_type);
}
