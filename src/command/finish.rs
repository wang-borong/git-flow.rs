use anyhow::Result;

use serde::{Deserialize, Serialize};

use crate::{
    config::definition::{BranchType, Strategy, TargetBranch},
    config::read::read_config,
    echo::Echo,
    git::Git,
    rules::{validate_merge_allowed, validate_strategy},
    utils::run_hook,
};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub cleanup_customer: bool,
}

pub fn finish_task(
    branch_name: String,
    branch_type: BranchType,
    opts: FinishOptions,
    config_path: Option<std::path::PathBuf>,
) {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    // -- load config for safety rule validation --
    let config = match read_config(config_path) {
        Ok(cfg) => cfg,
        Err(err) => {
            Echo::error(format!("Failed to read config: {}", err));
            return;
        }
    };
    let all_branch_types = config.branch_types.clone();
    let base_branch = config.base_branch;

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
            // ISSUE-F2: Handle invalid regex gracefully instead of panicking
            let regex = match regex::Regex::new(&y.name) {
                Ok(r) => r,
                Err(e) => {
                    Echo::error(format!(
                        "Invalid regex in target branch config '{}': {}",
                        y.name, e
                    ));
                    continue;
                }
            };
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
    if resolve_target_branches(
        &git,
        &branch_name,
        &target_branches,
        &target_branches,
        &branch_type,
        &opts,
        &all_branch_types,
        base_branch.as_deref(),
    )
    .is_err()
    {
        return;
    }

    // -- complete remainder of finish flow --
    complete_finish_flow(
        &git,
        &branch_name,
        &branch_type,
        &opts,
        &target_branches,
        false,
    );
}

pub fn complete_finish_flow(
    git: &Git,
    branch_name: &str,
    branch_type: &BranchType,
    opts: &FinishOptions,
    target_branches: &[TargetBranch],
    is_continue: bool,
) {
    use std::io::Write;

    // -- push target branches if configured or requested --
    if opts.push {
        push_target_branches(git, branch_type, target_branches);
    }

    // -- create tag if requested or if --bump is specified --
    if opts.tag.is_some() || opts.bump.is_some() {
        create_finish_tag(git, branch_name, branch_type, opts);
    }

    // ISSUE-F5 fix: In non-interactive environments (CI/piped), default to deleting the branch.
    // In interactive terminals, prompt the user if is_continue.
    if !opts.keep {
        use std::io::IsTerminal;
        let should_delete = if is_continue && std::io::stdin().is_terminal() {
            print!(
                "Do you want to delete the source branch '{}'? [y/N]: ",
                branch_name
            );
            let _ = std::io::stdout().flush();
            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_ok() {
                let trimmed = input.trim().to_lowercase();
                trimmed == "y" || trimmed == "yes"
            } else {
                false
            }
        } else {
            // Non-interactive: always delete (unless --keep was specified)
            true
        };

        if should_delete {
            let finish = Echo::progress(format!("delete branch {}", &branch_name));
            match git.del_local_branch(branch_name) {
                Err(err) => {
                    finish(false, &err.to_string());
                }
                Ok(_) => finish(true, &format!("delete branch {}", &branch_name)),
            }
        } else {
            Echo::info(format!("keeping branch {}", &branch_name));
        }
    } else {
        Echo::info(format!("keeping branch {}", &branch_name));
    }

    // -- run after finish hook --
    if !opts.no_verify {
        let _ = run_hook(branch_type.after_finish.clone(), branch_name, branch_type);
    }

    // -- auto cleanup customer branch if requested --
    if opts.cleanup_customer {
        if let Ok(Some((customer_branch, mut commits))) = git.load_generalize_state() {
            let finish = Echo::progress(format!("auto-cleanup on {}", customer_branch));
            // switch to customer branch
            if let Err(err) = git.switch(&customer_branch) {
                finish(
                    false,
                    &format!("failed to switch to {}: {}", customer_branch, err),
                );
            } else {
                // revert commits (in reverse order)
                commits.reverse();
                match git.revert(commits.clone()) {
                    Ok(_) => {
                        finish(
                            true,
                            &format!("reverted {} commits on {}", commits.len(), customer_branch),
                        );
                        // clear state
                        let _ = git.clear_generalize_state();

                        // Switch back to main branch (or the first target branch)
                        if let Some(tb) = target_branches.first() {
                            let _ = git.switch(&tb.name);
                        }
                    }
                    Err(_err) => {
                        let _ = git.revert_abort();
                        finish(false, "revert conflicts, aborted auto-cleanup");
                        // switch back
                        if let Some(tb) = target_branches.first() {
                            let _ = git.switch(&tb.name);
                        }
                        Echo::info(format!(
                            "Auto-cleanup failed. Please manually switch to {} and revert these commits to avoid future sync conflicts:\n{}",
                            customer_branch,
                            commits.join(", ")
                        ));
                    }
                }
            }
        } else {
            Echo::info("No generalize state found to cleanup");
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn resolve_target_branches(
    git: &Git,
    branch_name: &str,
    full_target_branches: &[TargetBranch],
    remaining_targets: &[TargetBranch],
    branch_type: &BranchType,
    opts: &FinishOptions,
    all_branch_types: &[BranchType],
    base_branch: Option<&str>,
) -> Result<()> {
    // Infer main branch: respects global configuration and does auto-detection
    let main_branch = infer_main_branch(branch_type, base_branch);

    for (i, x) in remaining_targets.iter().enumerate() {
        // Safety: check if merge is allowed (using config-derived rules)
        if let Some(ref main) = main_branch {
            if let Err(err) = validate_merge_allowed(branch_name, &x.name, main, all_branch_types) {
                Echo::error(err.to_string());
                return Err(err);
            }
            if let Err(err) = validate_strategy(branch_name, &x.name, &x.strategy, main) {
                Echo::error(err.to_string());
                return Err(err);
            }
        }

        let res = match x.strategy {
            Strategy::Merge => merge(git, branch_name, &x.name, opts.merge_message.as_deref()),
            Strategy::Rebase => rebase(git, branch_name, &x.name),
            Strategy::CherryPick => cherry_pick(git, branch_name, &x.name),
            Strategy::Squash => {
                squash_merge(git, branch_name, &x.name, opts.squash_message.as_deref())
            }
        };

        if let Err(err) = res {
            let err_str = err.to_string();
            if err_str.contains("conflict") {
                // Save state!
                let state = crate::command::state::GitflowState::Finish {
                    branch_name: branch_name.to_string(),
                    branch_type: branch_type.clone(),
                    opts: opts.clone(),
                    target_branches: full_target_branches.to_vec(),
                    remaining_targets: remaining_targets[i..].to_vec(),
                };
                if let Err(save_err) = state.save(git) {
                    Echo::error(format!("Failed to save gitflow state: {}", save_err));
                } else {
                    Echo::info(
                        "Gitflow state saved. Resolve the conflict and run 'gitflow continue'.",
                    );
                }
            }
            return Err(err);
        }
    }
    Ok(())
}

/// Infer the main branch name from the branch type configuration.
fn infer_main_branch(branch_type: &BranchType, base_branch: Option<&str>) -> Option<String> {
    if let Some(base) = base_branch {
        return Some(base.to_string());
    }

    // Auto-detect master or main branch
    let auto_main = if let Ok(git) = Git::open() {
        if let Ok(branches) = git.get_local_branches() {
            if branches.iter().any(|b| b == "master") && !branches.iter().any(|b| b == "main") {
                "master".to_string()
            } else {
                "main".to_string()
            }
        } else {
            "main".to_string()
        }
    } else {
        "main".to_string()
    };

    match branch_type.name.as_str() {
        "feature" | "hotfix" | "release" | "generalize" | "general" => Some(auto_main),
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
        return Err(err);
    }

    let custom_msg =
        custom_template.map(|t| format_commit_message(t, source_branch, target_branch));
    let result = git.merge(source_branch, custom_msg.as_deref());
    if let Err(err) = result {
        finish(false, &err.to_string());
        return Err(err);
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
        return Err(err);
    }

    let result = git.rebase(source_branch);
    if let Err(err) = result {
        finish(false, &err.to_string());
        return Err(err);
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
            return Err(err);
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
        return Err(err);
    }

    let result = git.cherry_pick(commits);
    if let Err(err) = result {
        finish(false, &err.to_string());
        return Err(err);
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
        return Err(err);
    }

    let custom_msg =
        custom_template.map(|t| format_commit_message(t, source_branch, target_branch));
    let result = git.squash_merge(source_branch, custom_msg.as_deref());
    if let Err(err) = result {
        finish(false, &err.to_string());
        return Err(err);
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
        // ISSUE-F1: Default to NOT pushing if the `push` field is not explicitly set.
        // Previously used unwrap_or(true) which could silently push unintended branches.
        let should_push = tb.push.unwrap_or(false);
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
                    // ISSUE-F3: Replace {{NAME}} BEFORE {NAME} to avoid partial double-brace substitution
                    .replace("{{NAME}}", short_name)
                    .replace("{NAME}", short_name),
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
