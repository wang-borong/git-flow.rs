use anyhow::{bail, Result};
use regex::Regex;

use crate::{
    config::definition::{BranchType, Strategy, TargetBranch},
    echo::Echo,
    git::Git,
    rules::{validate_merge_allowed, validate_strategy},
    utils::run_hook,
};

#[allow(dead_code)]
pub struct FinishOptions {
    pub keep: bool,
    pub tag: Option<String>,
    pub squash: bool,
    pub push: bool,
    pub fetch: bool,
    pub bump: Option<String>,
    pub squash_message: Option<String>,
    pub merge_message: Option<String>,
    pub update_message: Option<String>,
    pub no_verify: bool,
    pub sign: bool,
    pub customer: Option<String>,
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

    // -- run before finish hook (skip if --no-verify is true) --
    if !opts.no_verify
        && run_hook(
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
    if resolve_target_branches(&git, &branch_name, &target_branches, &branch_type, &opts).is_err() {
        return;
    }

    // -- push target branches if configured or requested --
    if opts.push {
        push_target_branches(&git, &branch_type, &target_branches);
    }

    // -- create tag if requested or if --bump is specified --
    if opts.tag.is_some() || opts.bump.is_some() {
        create_finish_tag(&git, &branch_name, &branch_type, &opts);
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
    if !opts.no_verify {
        let _ = run_hook(branch_type.after_finish.clone(), &branch_name, &branch_type);
    }
}

fn resolve_target_branches(
    git: &Git,
    branch_name: &str,
    target_branches: &[TargetBranch],
    branch_type: &BranchType,
    opts: &FinishOptions,
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
                merge(git, branch_name, &x.name, opts.merge_message.as_deref())?;
            }
            Strategy::Rebase => {
                rebase(git, branch_name, &x.name)?;
            }
            Strategy::CherryPick => {
                cherry_pick(git, branch_name, &x.name)?;
            }
            Strategy::Squash => {
                squash_merge(git, branch_name, &x.name, opts.squash_message.as_deref())?;
            }
        }
    }
    Ok(())
}

/// Infer the main branch name from the branch type configuration.
/// For feature/hotfix/generalize/general types, `from` is typically "main".
fn infer_main_branch(branch_type: &BranchType) -> Option<String> {
    match branch_type.name.as_str() {
        "feature" | "hotfix" | "release" | "generalize" | "general" => {
            Some(branch_type.from.clone())
        }
        _ => None, // Customer-specific types don't need safety checks
    }
}

fn format_commit_message(template: &str, source_branch: &str, target_branch: &str) -> String {
    let full_source = format!("refs/heads/{}", source_branch);
    let full_target = format!("refs/heads/{}", target_branch);
    template
        .replace("%%", "\x00percent\x00")
        .replace("%B", &full_source)
        .replace("%b", source_branch)
        .replace("%P", &full_target)
        .replace("%p", target_branch)
        .replace("\x00percent\x00", "%")
}

fn merge(
    git: &Git,
    source_branch: &str,
    target_branch: &str,
    custom_template: Option<&str>,
) -> Result<()> {
    let finish = Echo::progress(format!("merge {} into {}", source_branch, target_branch));

    let result = git.switch(target_branch);
    if let Err(err) = result {
        finish(false, &err.to_string());
        bail!("");
    }

    let custom_msg =
        custom_template.map(|t| format_commit_message(t, source_branch, target_branch));
    let result = git.merge(source_branch, custom_msg.as_deref());
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

fn squash_merge(
    git: &Git,
    source_branch: &str,
    target_branch: &str,
    custom_template: Option<&str>,
) -> Result<()> {
    let finish = Echo::progress(format!(
        "squash merge {} into {}",
        source_branch, target_branch
    ));

    let result = git.switch(target_branch);
    if let Err(err) = result {
        finish(false, &err.to_string());
        bail!("");
    }

    let custom_msg =
        custom_template.map(|t| format_commit_message(t, source_branch, target_branch));
    let result = git.squash_merge(source_branch, custom_msg.as_deref());
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

fn create_finish_tag(git: &Git, branch_name: &str, branch_type: &BranchType, opts: &FinishOptions) {
    let tag_name = if let Some(ref bump_type) = opts.bump {
        let customer_name = opts.customer.as_deref();
        match crate::utils::find_latest_tag_and_bump(git, customer_name, bump_type) {
            Ok(t) => t,
            Err(err) => {
                Echo::error(format!("Failed to auto bump version tag: {}", err));
                return;
            }
        }
    } else {
        let custom_tag = opts.tag.as_deref().unwrap_or("");
        if !custom_tag.is_empty() {
            custom_tag.to_string()
        } else {
            let prefix = format!("{}/", branch_type.name);
            let short_name = branch_name.strip_prefix(&prefix).unwrap_or(branch_name);
            match &branch_type.tag_pattern {
                Some(p) => p
                    .replace("{NAME}", short_name)
                    .replace("{{NAME}}", short_name),
                None => short_name.to_string(),
            }
        }
    };

    let finish = Echo::progress(format!("create tag {}", tag_name));
    match git.create_tag(&tag_name, &format!("Release {}", tag_name)) {
        Err(err) => {
            finish(false, &err.to_string());
        }
        Ok(_) => finish(true, &format!("create tag {}", tag_name)),
    }
}
