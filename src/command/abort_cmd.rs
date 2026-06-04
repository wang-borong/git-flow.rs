use crate::{echo::Echo, git::Git};

pub fn abort_operation() {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    if git.is_rebase_in_progress() {
        let finish = Echo::progress("abort rebase");
        match git.rebase_abort() {
            Err(err) => {
                finish(false, &err.to_string());
            }
            Ok(_) => finish(true, "rebase aborted"),
        }
    } else if git.is_merge_in_progress() {
        let finish = Echo::progress("abort merge");
        match git.merge_abort() {
            Err(err) => {
                finish(false, &err.to_string());
            }
            Ok(_) => finish(true, "merge aborted"),
        }
    } else {
        Echo::info("no rebase or merge in progress");
    }
}
