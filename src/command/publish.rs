use crate::{
    config::definition::BranchType,
    echo::Echo,
    git::Git,
    utils::run_hook,
};

pub fn publish_branch(branch_name: String, branch_type: BranchType) {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    // -- determine remote --
    let remote = match &branch_type.remote {
        Some(r) => r.clone(),
        None => {
            // try to auto-detect: use the first remote
            match git.get_remote_repos() {
                Ok(repos) if !repos.is_empty() => repos[0].clone(),
                Ok(_) => {
                    Echo::error("no remote configured and no remotes found");
                    return;
                }
                Err(err) => {
                    Echo::error(err.to_string());
                    return;
                }
            }
        }
    };

    // -- run before publish hook --
    if run_hook(
        branch_type.before_publish.clone(),
        &branch_name,
        &branch_type,
    )
    .is_err()
    {
        return;
    }

    // -- push to remote --
    let finish = Echo::progress(format!(
        "push {} to {}/{}",
        branch_name, remote, branch_name
    ));
    match git.push_branch(&remote, &branch_name, &branch_name) {
        Err(err) => {
            finish(false, &err.to_string());
            return;
        }
        Ok(_) => finish(
            true,
            &format!("push {} to {}/{}", branch_name, remote, branch_name),
        ),
    }

    // -- run after publish hook --
    let _ = run_hook(
        branch_type.after_publish.clone(),
        &branch_name,
        &branch_type,
    );
}
