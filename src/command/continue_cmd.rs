use crate::{echo::Echo, git::Git};

pub fn continue_operation() {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    if git.is_rebase_in_progress() {
        let finish = Echo::progress("continue rebase");
        match git.rebase_continue() {
            Err(err) => {
                finish(false, &err.to_string());
            }
            Ok(_) => finish(true, "rebase continued"),
        }
    } else if git.is_merge_in_progress() {
        let finish = Echo::progress("continue merge");
        match git.merge_continue() {
            Err(err) => {
                finish(false, &err.to_string());
            }
            Ok(_) => finish(true, "merge continued"),
        }
    } else if git.is_cherrypick_in_progress() {
        let finish = Echo::progress("continue cherry-pick");
        match git.cherrypick_continue() {
            Err(err) => {
                finish(false, &err.to_string());
            }
            Ok(_) => finish(true, "cherry-pick continued"),
        }
    } else {
        Echo::info("no rebase, merge or cherry-pick in progress");
    }
}
