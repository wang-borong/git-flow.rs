use crate::{config::definition::BranchType, echo::Echo, git::Git, utils::run_hook};

#[allow(dead_code)]
pub fn rebase_branch(branch_name: String, branch_type: BranchType) {
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

    // -- run before rebase hook --
    if run_hook(
        branch_type.before_rebase.clone(),
        &branch_name,
        &branch_type,
    )
    .is_err()
    {
        return;
    }

    // -- switch to the branch --
    let finish = Echo::progress(format!("switch to branch {}", &branch_name));
    if let Err(err) = git.switch(&branch_name) {
        finish(false, &err.to_string());
        return;
    }
    finish(true, &format!("switch to branch {}", &branch_name));

    // -- rebase onto source --
    let finish = Echo::progress(format!("rebase {} onto {}", branch_name, branch_type.from));
    match git.rebase(&branch_type.from) {
        Err(err) => {
            finish(false, &err.to_string());
            Echo::info("resolve conflicts, then run `gitflow continue`");
            return;
        }
        Ok(_) => finish(
            true,
            &format!("rebase {} onto {}", branch_name, branch_type.from),
        ),
    }

    // -- run after rebase hook --
    let _ = run_hook(branch_type.after_rebase.clone(), &branch_name, &branch_type);
}
