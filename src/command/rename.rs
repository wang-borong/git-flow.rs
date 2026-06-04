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
    if branches.iter().all(|x| x.as_str() != old_name) {
        Echo::error(format!("branch {} not found", old_name));
        return;
    }

    if branches.iter().any(|x| x.as_str() == new_name) {
        Echo::error(format!("branch {} already exists", new_name));
        return;
    }

    let finish = Echo::progress(format!("rename branch {} to {}", old_name, new_name));
    match git.rename_branch(&old_name, &new_name) {
        Err(err) => {
            finish(false, &err.to_string());
        }
        Ok(_) => {
            finish(true, &format!("rename branch {} to {}", old_name, new_name));
        }
    }
}
