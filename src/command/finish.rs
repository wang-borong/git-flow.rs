use anyhow::{bail, Result};
use regex::Regex;

use crate::{
    config::definition::{BranchType, Strategy, TargetBranch},
    echo::Echo,
    git::Git,
    rules::{validate_merge_allowed, validate_strategy},
    utils::run_hook,
};

pub struct FinishOptions {
    pub keep: bool,
    pub tag: bool,
    pub squash: bool,
    pub push: bool,
    pub fetch: bool,
}

pub fn finish_task(branch_name: String, branch_type: BranchType, opts: FinishOptions) {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    // -- fetch source branch if requested --
    if opts.fetch {
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
    if branches.iter().all(|x| x.as_str() != branch_name) {
        Echo::error(format!("branch {} is not found", branch_name));
        return;
    }

    // -- run before finish hook --
    if run_hook(
        branch_type.before_finish.clone(),
        &branch_name,
        &branch_type,
    )
    .is_err()
    {
        return;
    }

    // -- collect target branches --
    let mut target_branches = Vec::<TargetBranch>::new();
    branches.iter().for_each(|x| {
        for y in branch_type.to.iter() {
            let regex = Regex::new(&y.name).unwrap();
            if regex.is_match(x) {
                target_branches.push(TargetBranch {
                    name: x.to_string(),
                    strategy: if opts.squash {
                        Strategy::Squash
                    } else {
                        y.strategy.clone()
                    },
                    push: y.push,
                    tag: y.tag,
                });
                break;
            }
        }
    });

    // -- resolve target branches --
    if resolve_target_branches(&git, &branch_name, &target_branches, &branch_type).is_err() {
        return;
    }

    // -- push target branches if configured or requested --
    if opts.push {
        push_target_branches(&git, &branch_type, &target_branches);
    }

    // -- create tag if requested --
    if opts.tag {
        create_finish_tag(&git, &branch_name, &branch_type);
    }

    // -- delete branch (unless --keep) --
    if !opts.keep {
        let finish = Echo::progress(format!("delete branch {}", &branch_name));
        match git.del_local_branch(&branch_name) {
            Err(err) => {
                finish(false, &err.to_string());
                return;
            }
            Ok(_) => finish(true, &format!("delete branch {}", &branch_name)),
        }
    } else {
        Echo::info(format!("keeping branch {}", &branch_name));
    }

    // -- run after finish hook --
    let _ = run_hook(branch_type.after_finish.clone(), &branch_name, &branch_type);
}

fn resolve_target_branches(
    git: &Git,
    branch_name: &str,
    target_branches: &[TargetBranch],
    branch_type: &BranchType,
) -> Result<()> {
    // Infer main branch: for feature/hotfix types, the `from` field is typically "main"
    let main_branch = infer_main_branch(branch_type);

    for x in target_branches.iter() {
        // Safety: check if merge is allowed
        if let Some(ref main) = main_branch {
            if let Err(err) = validate_merge_allowed(branch_name, &x.name, main) {
                Echo::error(err.to_string());
                bail!("");
            }
            if let Err(err) = validate_strategy(branch_name, &x.name, &x.strategy, main) {
                Echo::error(err.to_string());
                bail!("");
            }
        }

        match x.strategy {
            Strategy::Merge => {
                merge(git, branch_name, &x.name)?;
            }
            Strategy::Rebase => {
                rebase(git, branch_name, &x.name)?;
            }
            Strategy::CherryPick => {
                cherry_pick(git, branch_name, &x.name)?;
            }
            Strategy::Squash => {
                squash_merge(git, branch_name, &x.name)?;
            }
        }
    }
    Ok(())
}

/// Infer the main branch name from the branch type configuration.
/// For feature/hotfix/generalize types, `from` is typically "main".
fn infer_main_branch(branch_type: &BranchType) -> Option<String> {
    match branch_type.name.as_str() {
        "feature" | "hotfix" | "release" | "generalize" => Some(branch_type.from.clone()),
        _ => None, // Customer-specific types don't need safety checks
    }
}

fn merge(git: &Git, source_branch: &str, target_branch: &str) -> Result<()> {
    let finish = Echo::progress(format!("merge {} into {}", source_branch, target_branch));

    let result = git.switch(target_branch);
    if let Err(err) = result {
        finish(false, &err.to_string());
        bail!("");
    }

    let result = git.merge(source_branch);
    if let Err(err) = result {
        finish(false, &err.to_string());
        bail!("");
    }

    finish(
        true,
        &format!("merge {} into {}", source_branch, target_branch),
    );
    Ok(())
}

fn rebase(git: &Git, source_branch: &str, target_branch: &str) -> Result<()> {
    let finish = Echo::progress(format!("rebase {} onto {}", target_branch, source_branch));

    let result = git.switch(target_branch);
    if let Err(err) = result {
        finish(false, &err.to_string());
        bail!("");
    }

    let result = git.rebase(source_branch);
    if let Err(err) = result {
        finish(false, &err.to_string());
        bail!("");
    }

    finish(
        true,
        &format!("rebase {} onto {}", target_branch, source_branch),
    );
    Ok(())
}

fn cherry_pick(git: &Git, source_branch: &str, target_branch: &str) -> Result<()> {
    let commits = match git.diff_commits(source_branch, target_branch) {
        Err(err) => {
            Echo::error(err.to_string());
            bail!("");
        }
        Ok(commits_v) => commits_v,
    };
    if commits.is_empty() {
        Echo::success(format!("no commits to cherry pick to {}", target_branch));
        return Ok(());
    }
    let msg = if commits.len() == 1 {
        format!("cherry pick commit {} to {}", &commits[0], target_branch)
    } else {
        format!(
            "cherry pick commits {}..{} to {}",
            &commits[0],
            &commits.last().unwrap(),
            target_branch
        )
    };
    let finish = Echo::progress(&msg);

    let result = git.switch(target_branch);
    if let Err(err) = result {
        finish(false, &err.to_string());
        bail!("");
    }

    let result = git.cherry_pick(commits);
    if let Err(err) = result {
        finish(false, &err.to_string());
        bail!("");
    }

    finish(true, &msg);
    Ok(())
}

fn squash_merge(git: &Git, source_branch: &str, target_branch: &str) -> Result<()> {
    let finish = Echo::progress(format!(
        "squash merge {} into {}",
        source_branch, target_branch
    ));

    let result = git.switch(target_branch);
    if let Err(err) = result {
        finish(false, &err.to_string());
        bail!("");
    }

    let result = git.squash_merge(source_branch);
    if let Err(err) = result {
        finish(false, &err.to_string());
        bail!("");
    }

    finish(
        true,
        &format!("squash merge {} into {}", source_branch, target_branch),
    );
    Ok(())
}

fn push_target_branches(git: &Git, branch_type: &BranchType, target_branches: &[TargetBranch]) {
    let remote = match &branch_type.remote {
        Some(r) => r.clone(),
        None => return,
    };

    for tb in target_branches {
        // Check if this target has push enabled, or if --push was used (push all)
        let should_push = tb.push.unwrap_or(true); // --push pushes all targets
        if should_push {
            let finish = Echo::progress(format!("push {} to {}", tb.name, remote));
            match git.push_branch(&remote, &tb.name, &tb.name) {
                Err(err) => {
                    finish(false, &err.to_string());
                }
                Ok(_) => finish(true, &format!("push {} to {}", tb.name, remote)),
            }
        }
    }
}

fn create_finish_tag(git: &Git, branch_name: &str, branch_type: &BranchType) {
    let tag_pattern = match &branch_type.tag_pattern {
        Some(p) => p.clone(),
        None => {
            // Auto-generate tag name from branch name
            let prefix = format!("{}/", branch_type.name);
            let tag_name = branch_name.strip_prefix(&prefix).unwrap_or(branch_name);
            format!("v{}", tag_name)
        }
    };

    // Replace {NAME} placeholder
    let prefix = format!("{}/", branch_type.name);
    let short_name = branch_name.strip_prefix(&prefix).unwrap_or(branch_name);
    let tag_name = tag_pattern.replace("{NAME}", short_name);

    let finish = Echo::progress(format!("create tag {}", tag_name));
    match git.create_tag(&tag_name, &format!("Release {}", tag_name)) {
        Err(err) => {
            finish(false, &err.to_string());
        }
        Ok(_) => finish(true, &format!("create tag {}", tag_name)),
    }
}
