use crate::{command::state::GitflowState, echo::Echo, git::Git};

pub fn abort_operation() {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    // Load state BEFORE clearing, so we know where to switch back to
    let state = GitflowState::load(&git).ok().flatten();

    if git.is_rebase_in_progress() {
        let finish = Echo::progress("abort rebase");
        match git.rebase_abort() {
            Err(err) => {
                finish(false, &err.to_string());
            }
            Ok(_) => {
                finish(true, "rebase aborted");
            }
        }
    } else if git.is_merge_in_progress() {
        let finish = Echo::progress("abort merge");
        match git.merge_abort() {
            Err(err) => {
                finish(false, &err.to_string());
            }
            Ok(_) => {
                finish(true, "merge aborted");
            }
        }
    } else if git.is_cherrypick_in_progress() {
        let finish = Echo::progress("abort cherry-pick");
        match git.cherrypick_abort() {
            Err(err) => {
                finish(false, &err.to_string());
            }
            Ok(_) => {
                finish(true, "cherry-pick aborted");
            }
        }
    } else {
        Echo::info("no rebase, merge or cherry-pick in progress");
    }

    // Always attempt to clear gitflow state file on abort
    if let Err(err) = GitflowState::clear(&git) {
        Echo::error(format!("Failed to clear gitflow state: {}", err));
    }

    // ISSUE-A1 fix: Switch back to source branch (the branch being finished/synced)
    // so the developer is not left on the target branch (e.g., `main`)
    let source_branch = match &state {
        Some(GitflowState::Finish { branch_name, .. }) => Some(branch_name.clone()),
        Some(GitflowState::Start { branch_name, .. }) => Some(branch_name.clone()),
        Some(GitflowState::Sync {
            original_branch, ..
        }) => {
            // For sync, switch back to original branch (not the customer branch)
            Some(original_branch.clone())
        }
        None => None,
    };

    if let Some(ref branch) = source_branch {
        let finish = Echo::progress(format!("restore branch {}", branch));
        match git.switch(branch) {
            Ok(_) => finish(true, &format!("restored to branch {}", branch)),
            Err(err) => finish(false, &format!("could not restore to {}: {}", branch, err)),
        }
    }
}
