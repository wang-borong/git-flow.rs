use std::io::IsTerminal;

use crate::{
    command::state::GitflowState, config::definition::Strategy, echo::Echo, git::Git,
    utils::run_hook,
};

pub fn continue_operation() {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    let state = match GitflowState::load(&git) {
        Err(err) => {
            Echo::error(format!("Failed to load gitflow state: {}", err));
            return;
        }
        Ok(s) => s,
    };

    let mut native_continued = false;

    if git.is_rebase_in_progress() {
        let finish = Echo::progress("continue rebase");
        match git.rebase_continue() {
            Err(err) => {
                finish(false, &err.to_string());
                return;
            }
            Ok(_) => {
                finish(true, "rebase continued");
                native_continued = true;
            }
        }
    } else if git.is_merge_in_progress() {
        let finish = Echo::progress("continue merge");
        match git.merge_continue() {
            Err(err) => {
                finish(false, &err.to_string());
                return;
            }
            Ok(_) => {
                finish(true, "merge continued");
                native_continued = true;
            }
        }
    } else if git.is_cherrypick_in_progress() {
        let finish = Echo::progress("continue cherry-pick");
        match git.cherrypick_continue() {
            Err(err) => {
                finish(false, &err.to_string());
                return;
            }
            Ok(_) => {
                finish(true, "cherry-pick continued");
                native_continued = true;
            }
        }
    } else if let Some(GitflowState::Finish {
        ref remaining_targets,
        ..
    }) = state
    {
        if !remaining_targets.is_empty() && remaining_targets[0].strategy == Strategy::Squash {
            let finish = Echo::progress("continue squash merge");
            match git.has_conflicts() {
                Ok(true) => {
                    finish(
                        false,
                        "Conflicts still exist in the working tree. Please resolve them first.",
                    );
                    return;
                }
                Err(err) => {
                    finish(false, &err.to_string());
                    return;
                }
                Ok(false) => {}
            }

            match git.has_uncommitted_changes() {
                Ok(true) => {
                    let workdir = match git.workdir() {
                        Ok(d) => d,
                        Err(err) => {
                            finish(false, &err.to_string());
                            return;
                        }
                    };

                    // ISSUE-C1 fix: In non-interactive environments, use --no-edit to avoid hanging.
                    // In interactive terminals, open the editor for the user to write a commit message.
                    let is_interactive = std::io::stdin().is_terminal();

                    // Get squash message from saved state if available
                    let squash_msg = if let Some(GitflowState::Finish { ref opts, .. }) = state {
                        opts.squash_message.clone()
                    } else {
                        None
                    };

                    let mut cmd = std::process::Command::new("git");
                    cmd.arg("commit").current_dir(&workdir);

                    if let Some(ref msg) = squash_msg {
                        cmd.args(["-m", msg]);
                    } else if !is_interactive {
                        // Non-interactive: use --no-edit to prevent hanging
                        cmd.arg("--no-edit");
                    }
                    // else: interactive with no custom message → open editor (original behavior)

                    match cmd.status() {
                        Ok(status) if status.success() => {
                            finish(true, "squash merge committed");
                            native_continued = true;
                        }
                        Ok(_) => {
                            finish(false, "git commit failed");
                            return;
                        }
                        Err(err) => {
                            finish(false, &err.to_string());
                            return;
                        }
                    }
                }
                Ok(false) => {
                    finish(true, "working tree clean, proceeding");
                    native_continued = true;
                }
                Err(err) => {
                    finish(false, &err.to_string());
                    return;
                }
            }
        }
    }

    if let Some(s) = state {
        match s {
            GitflowState::Finish {
                branch_name,
                branch_type,
                opts,
                target_branches,
                remaining_targets,
            } => {
                // The current (first) target is now resolved.
                // We resolve the rest of the remaining targets.
                if remaining_targets.len() > 1 {
                    let next_targets = remaining_targets[1..].to_vec();
                    // Load config for safety rule validation
                    let config = crate::config::read::read_config(None).ok();
                    let all_branch_types = config
                        .as_ref()
                        .map(|c| c.branch_types.clone())
                        .unwrap_or_default();
                    let base_branch = config.and_then(|c| c.base_branch);

                    if crate::command::finish::resolve_target_branches(
                        &git,
                        &branch_name,
                        &target_branches,
                        &next_targets,
                        &branch_type,
                        &opts,
                        &all_branch_types,
                        base_branch.as_deref(),
                    )
                    .is_err()
                    {
                        // resolve_target_branches saved a new state (with the next remaining targets) if it failed.
                        // ISSUE-C2 fix: Inform user they need to continue again.
                        Echo::info("Next target branch also has a conflict. Resolve it and run 'gitflow continue' again.");
                        return;
                    }
                }

                // All target branches resolved! Complete the finish flow
                crate::command::finish::complete_finish_flow(
                    &git,
                    &branch_name,
                    &branch_type,
                    &opts,
                    &target_branches,
                    true, // is_continue = true (prompts for branch deletion)
                );

                // Clear the state
                let _ = GitflowState::clear(&git);
            }
            GitflowState::Start {
                branch_name,
                branch_type,
            } => {
                // The start cherry pick has succeeded. Run the after start hook
                let _ = run_hook(branch_type.after_start.clone(), &branch_name, &branch_type);
                // Clear the state
                let _ = GitflowState::clear(&git);
            }
            GitflowState::Sync {
                customer_branch,
                main_branch,
                push,
                remote,
                original_branch,
                ..
            } => {
                // The sync conflict has been resolved (via merge_continue or rebase_continue above).
                // Now push if needed and restore the original branch.
                let finish_sync = Echo::progress(format!("complete sync of {}", customer_branch));

                if push {
                    if let Some(ref remote_name) = remote {
                        let push_finish = Echo::progress(format!(
                            "push {} to {}/{}",
                            customer_branch, remote_name, customer_branch
                        ));
                        match git.push_branch(remote_name, &customer_branch, &customer_branch) {
                            Err(err) => {
                                push_finish(false, &err.to_string());
                            }
                            Ok(_) => push_finish(
                                true,
                                &format!(
                                    "push {} to {}/{}",
                                    customer_branch, remote_name, customer_branch
                                ),
                            ),
                        }
                    }
                }

                finish_sync(
                    true,
                    &format!("'{}' synced with '{}'", customer_branch, main_branch),
                );

                // Restore original branch
                if original_branch != customer_branch {
                    if let Err(err) = git.switch(&original_branch) {
                        Echo::error(format!(
                            "Failed to restore original branch '{}': {}",
                            original_branch, err
                        ));
                    }
                }

                // Clear the sync state
                let _ = GitflowState::clear(&git);
            }
        }
    } else if !native_continued {
        Echo::info("no rebase, merge or cherry-pick in progress");
    }
}
