use std::cell::RefCell;
use std::io::{self, Write};
use std::path::Path;

use anyhow::{bail, Result};
use git2::{build::CheckoutBuilder, Cred, RemoteCallbacks, Repository, StashFlags};

#[cfg(test)]
mod test;

mod branch;

pub struct Git {
    repo: RefCell<Repository>,
}

impl Git {
    pub fn open() -> Result<Self> {
        let repo = Repository::open_from_env()?;
        Ok(Self {
            repo: RefCell::new(repo),
        })
    }

    #[allow(dead_code)]
    pub fn from_repo(repo: Repository) -> Self {
        Self {
            repo: RefCell::new(repo),
        }
    }

    pub fn git_dir(&self) -> std::path::PathBuf {
        self.repo.borrow().path().to_path_buf()
    }
}

// # status
impl Git {
    pub fn git_installed() -> bool {
        Repository::open_from_env().is_ok()
    }
}

// # combine
impl Git {
    pub fn merge(&self, source_branch: &str, custom_msg: Option<&str>) -> Result<()> {
        if crate::utils::is_dry_run() {
            println!(
                "[Dry Run] git merge {} --no-ff{}",
                source_branch,
                custom_msg
                    .map(|m| format!(" -m \"{}\"", m))
                    .unwrap_or_default()
            );
            return Ok(());
        }
        let repo = self.repo.borrow();
        let source_ref = repo.find_branch(source_branch, git2::BranchType::Local)?;
        let source_oid = source_ref.get().target().unwrap();
        let source_annotated = repo.find_annotated_commit(source_oid)?;
        let mut merge_opts = git2::MergeOptions::new();
        let mut checkout_opts = CheckoutBuilder::new();
        repo.merge(
            &[&source_annotated],
            Some(&mut merge_opts),
            Some(&mut checkout_opts),
        )?;

        let index = repo.index()?;
        if index.has_conflicts() {
            bail!("merge conflict detected, resolve manually");
        }

        let sig = repo.signature()?;
        let head_commit = repo.head()?.peel_to_commit()?;
        let source_commit = repo.find_commit(source_oid)?;
        let tree = repo.find_tree(repo.index()?.write_tree()?)?;
        let msg = if let Some(m) = custom_msg {
            m.to_string()
        } else {
            let target_branch = self.current_branch()?;
            let initial_msg = self.build_merge_message(source_branch, &target_branch, false)?;
            self.edit_commit_message(&initial_msg)?
        };

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &msg,
            &tree,
            &[&head_commit, &source_commit],
        )?;

        repo.cleanup_state()?;
        Ok(())
    }

    pub fn rebase(&self, base_branch: &str) -> Result<()> {
        if crate::utils::is_dry_run() {
            println!("[Dry Run] git rebase {}", base_branch);
            return Ok(());
        }
        let repo = self.repo.borrow();
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("No workdir found"))?;
        let output = std::process::Command::new("git")
            .arg("rebase")
            .arg(base_branch)
            .current_dir(workdir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let index = repo.index()?;
            if index.has_conflicts() {
                bail!(
                    "rebase conflict detected, resolve manually\n{}",
                    stderr.trim()
                );
            } else {
                bail!("rebase failed: {}", stderr.trim());
            }
        }
        Ok(())
    }

    pub fn cherry_pick(&self, commits: Vec<String>) -> Result<()> {
        if crate::utils::is_dry_run() {
            println!("[Dry Run] git cherry-pick {}", commits.join(" "));
            return Ok(());
        }
        let repo = self.repo.borrow();
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("No workdir found"))?;
        let output = std::process::Command::new("git")
            .arg("cherry-pick")
            .args(&commits)
            .current_dir(workdir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let index = repo.index()?;
            if index.has_conflicts() {
                bail!(
                    "cherry-pick conflict detected, resolve manually\n{}",
                    stderr.trim()
                );
            } else {
                bail!("cherry-pick failed: {}", stderr.trim());
            }
        }
        Ok(())
    }

    pub fn revert(&self, commits: Vec<String>) -> Result<()> {
        if crate::utils::is_dry_run() {
            println!("[Dry Run] git revert --no-edit {}", commits.join(" "));
            return Ok(());
        }
        let repo = self.repo.borrow();
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("No workdir found"))?;
        let output = std::process::Command::new("git")
            .arg("revert")
            .arg("--no-edit")
            .args(&commits)
            .current_dir(workdir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("revert failed or conflicted: {}", stderr.trim());
        }
        Ok(())
    }

    pub fn revert_abort(&self) -> Result<()> {
        if crate::utils::is_dry_run() {
            println!("[Dry Run] git revert --abort");
            return Ok(());
        }
        let repo = self.repo.borrow();
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("No workdir found"))?;
        let output = std::process::Command::new("git")
            .args(["revert", "--abort"])
            .current_dir(workdir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("revert abort failed: {}", stderr.trim());
        }
        Ok(())
    }

    /// Squash merge: merge the source tree into the target without preserving
    /// individual commits. Creates a single commit with all changes.
    pub fn squash_merge(&self, source_branch: &str, custom_msg: Option<&str>) -> Result<()> {
        if crate::utils::is_dry_run() {
            println!(
                "[Dry Run] git merge {} --squash{}",
                source_branch,
                custom_msg
                    .map(|m| format!(" -m \"{}\"", m))
                    .unwrap_or_default()
            );
            return Ok(());
        }
        let repo = self.repo.borrow();
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("No workdir found"))?;
        let output = std::process::Command::new("git")
            .args(["merge", "--squash", source_branch])
            .current_dir(workdir)
            .output()?;
        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut index = repo.index()?;
            let _ = index.read(true); // Force reload index from disk
            if index.has_conflicts() {
                bail!(
                    "squash merge conflict detected, resolve manually\n{}",
                    stdout.trim()
                );
            } else {
                bail!(
                    "squash merge failed:\nstdout: {}\nstderr: {}",
                    stdout.trim(),
                    stderr.trim()
                );
            }
        }

        // Merge succeeded, commit the squash
        let msg = if let Some(m) = custom_msg {
            m.to_string()
        } else {
            let target_branch = self.current_branch()?;
            let initial_msg = self.build_merge_message(source_branch, &target_branch, true)?;
            self.edit_commit_message(&initial_msg)?
        };

        let commit_output = std::process::Command::new("git")
            .args(["commit", "-m", &msg])
            .current_dir(workdir)
            .output()?;
        if !commit_output.status.success() {
            let stderr = String::from_utf8_lossy(&commit_output.stderr);
            bail!("squash merge commit failed: {}", stderr.trim());
        }

        Ok(())
    }
}

// # other
impl Git {
    pub fn switch(&self, target_branch: &str) -> Result<()> {
        if crate::utils::is_dry_run() {
            println!("[Dry Run] git checkout {}", target_branch);
            return Ok(());
        }
        let repo = self.repo.borrow();
        let branch = repo.find_branch(target_branch, git2::BranchType::Local)?;
        let commit = branch.get().peel(git2::ObjectType::Commit)?;

        let mut opts = CheckoutBuilder::new();
        opts.safe();
        repo.checkout_tree(&commit, Some(&mut opts))?;

        let refname = format!("refs/heads/{}", target_branch);
        repo.set_head(&refname)?;

        let mut index = repo.index()?;
        index.write()?;

        Ok(())
    }

    /// commits on source_branch but not on target_branch
    pub fn diff_commits(&self, source_branch: &str, target_branch: &str) -> Result<Vec<String>> {
        let repo = self.repo.borrow();
        let source_ref = repo.find_branch(source_branch, git2::BranchType::Local)?;
        let target_ref = repo.find_branch(target_branch, git2::BranchType::Local)?;
        let source_oid = source_ref.get().target().unwrap();
        let target_oid = target_ref.get().target().unwrap();

        let mut walk = repo.revwalk()?;
        walk.push(source_oid)?;
        walk.hide(target_oid)?;

        let mut commits = Vec::new();
        for oid in walk {
            let oid = oid?;
            commits.push(oid.to_string());
        }
        Ok(commits)
    }

    /// commit messages on source_branch but not on target_branch
    pub fn diff_commit_messages(
        &self,
        source_branch: &str,
        target_branch: &str,
    ) -> Result<Vec<String>> {
        let repo = self.repo.borrow();
        let source_ref = repo.find_branch(source_branch, git2::BranchType::Local)?;
        let target_ref = repo.find_branch(target_branch, git2::BranchType::Local)?;
        let source_oid = source_ref.get().target().unwrap();
        let target_oid = target_ref.get().target().unwrap();

        let mut walk = repo.revwalk()?;
        walk.push(source_oid)?;
        walk.hide(target_oid)?;

        let mut messages = Vec::new();
        for oid in walk {
            let oid = oid?;
            let commit = repo.find_commit(oid)?;
            if let Ok(Some(msg)) = commit.summary() {
                messages.push(msg.to_string());
            }
        }
        // Reverse so that oldest commit is at the top
        messages.reverse();
        Ok(messages)
    }

    pub fn build_merge_message(
        &self,
        source_branch: &str,
        target_branch: &str,
        is_squash: bool,
    ) -> Result<String> {
        let mut msg = if is_squash {
            format!("Squash merge branch '{}'\n\n", source_branch)
        } else {
            format!("Merge branch '{}'\n\n", source_branch)
        };

        if let Ok(commits) = self.diff_commit_messages(source_branch, target_branch) {
            for c_msg in commits {
                msg.push_str(&format!("* {}\n", c_msg));
            }
        }

        msg.push_str("\n# Please enter the commit message for your changes. Lines starting\n# with '#' will be ignored, and an empty message aborts the commit.\n");

        Ok(msg)
    }

    pub fn edit_commit_message(&self, initial_message: &str) -> Result<String> {
        let repo = self.repo.borrow();
        let edit_path = repo.path().join("GITFLOW_EDITMSG");
        std::fs::write(&edit_path, initial_message)?;

        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());

        let status = std::process::Command::new(&editor)
            .arg(&edit_path)
            .status()?;

        if !status.success() {
            let _ = std::fs::remove_file(&edit_path);
            bail!("Editor exited with error status");
        }

        let edited = std::fs::read_to_string(&edit_path)?;
        let _ = std::fs::remove_file(&edit_path);

        let mut final_msg = String::new();
        for line in edited.lines() {
            if !line.trim_start().starts_with('#') {
                final_msg.push_str(line);
                final_msg.push('\n');
            }
        }

        let final_msg = final_msg.trim().to_string();
        if final_msg.is_empty() {
            bail!("Aborting commit due to empty commit message.");
        }

        Ok(final_msg)
    }

    /// output commits on source_branch but not on target_branch
    pub fn diff_logs(&self, source_branch: &str, target_branch: &str) -> Result<()> {
        let repo = self.repo.borrow();
        let source_ref = repo.find_branch(source_branch, git2::BranchType::Local)?;
        let target_ref = repo.find_branch(target_branch, git2::BranchType::Local)?;
        let source_oid = source_ref.get().target().unwrap();
        let target_oid = target_ref.get().target().unwrap();

        let mut walk = repo.revwalk()?;
        walk.push(source_oid)?;
        walk.hide(target_oid)?;

        let stdout = io::stdout();
        let mut out = stdout.lock();
        for oid in walk {
            let oid = oid?;
            let commit = repo.find_commit(oid)?;
            writeln!(out, "commit {}", oid)?;
            if let Ok(author) = commit.author().name() {
                writeln!(out, "Author: {}", author)?;
            }
            writeln!(out)?;
            if let Ok(msg) = commit.message() {
                for line in msg.lines() {
                    writeln!(out, "    {}", line)?;
                }
            }
            writeln!(out)?;
        }
        Ok(())
    }

    pub fn get_remote_repos(&self) -> Result<Vec<String>> {
        let repo = self.repo.borrow();
        let remotes = repo.remotes()?;
        let mut names = Vec::new();
        for entry in remotes.iter() {
            if let Some(name) = entry? {
                names.push(name.to_string());
            }
        }
        Ok(names)
    }

    /// Get the current branch name.
    pub fn current_branch(&self) -> Result<String> {
        let repo = self.repo.borrow();
        let head = repo.head()?;
        let name = head.shorthand().unwrap_or("HEAD").to_string();
        Ok(name)
    }
}

// # stash & conflict state
#[allow(dead_code)]
impl Git {
    pub fn workdir(&self) -> Result<std::path::PathBuf> {
        let repo = self.repo.borrow();
        let path = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("No workdir found"))?;
        Ok(path.to_path_buf())
    }

    pub fn has_conflicts(&self) -> Result<bool> {
        let repo = self.repo.borrow();
        let index = repo.index()?;
        Ok(index.has_conflicts())
    }

    /// Check if there are uncommitted changes in the working directory.
    pub fn has_uncommitted_changes(&self) -> Result<bool> {
        let repo = self.repo.borrow();
        let statuses = repo.statuses(Some(git2::StatusOptions::new().include_untracked(true)))?;
        Ok(!statuses.is_empty())
    }

    /// Stash current changes. Returns the stash OID.
    pub fn stash(&self, message: &str) -> Result<String> {
        let mut repo = self.repo.borrow_mut();
        let sig = repo.signature()?.clone();
        let oid = repo.stash_save(&sig, message, Some(StashFlags::DEFAULT))?;
        Ok(oid.to_string())
    }

    /// Pop the most recent stash entry.
    pub fn stash_pop(&self) -> Result<()> {
        let mut repo = self.repo.borrow_mut();
        repo.stash_pop(0, None)?;
        Ok(())
    }

    /// Check if a rebase is currently in progress.
    pub fn is_rebase_in_progress(&self) -> bool {
        let repo = self.repo.borrow();
        repo.state() == git2::RepositoryState::Rebase
            || repo.state() == git2::RepositoryState::RebaseInteractive
            || repo.state() == git2::RepositoryState::RebaseMerge
    }

    /// Check if a merge is currently in progress.
    pub fn is_merge_in_progress(&self) -> bool {
        let repo = self.repo.borrow();
        repo.state() == git2::RepositoryState::Merge
    }

    /// Check if a cherry-pick is currently in progress.
    pub fn is_cherrypick_in_progress(&self) -> bool {
        let repo = self.repo.borrow();
        repo.state() == git2::RepositoryState::CherryPick
            || repo.state() == git2::RepositoryState::CherryPickSequence
    }

    /// Continue a rebase after conflict resolution.
    pub fn rebase_continue(&self) -> Result<()> {
        if crate::utils::is_dry_run() {
            println!("[Dry Run] git rebase --continue");
            return Ok(());
        }
        let repo = self.repo.borrow();
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("No workdir found"))?;
        use std::io::IsTerminal;
        let is_interactive = std::io::stdin().is_terminal();
        let mut cmd = std::process::Command::new("git");
        cmd.args(["rebase", "--continue"]).current_dir(workdir);
        if !is_interactive {
            cmd.env("GIT_EDITOR", "true");
        }
        let status = cmd.status()?;
        if !status.success() {
            bail!("rebase continue failed");
        }
        Ok(())
    }

    /// Abort a rebase in progress, returning to the pre-rebase state.
    pub fn rebase_abort(&self) -> Result<()> {
        if crate::utils::is_dry_run() {
            println!("[Dry Run] git rebase --abort");
            return Ok(());
        }
        let repo = self.repo.borrow();
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("No workdir found"))?;
        let output = std::process::Command::new("git")
            .args(["rebase", "--abort"])
            .current_dir(workdir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("rebase abort failed: {}", stderr.trim());
        }
        Ok(())
    }

    /// Continue a merge after conflict resolution.
    pub fn merge_continue(&self) -> Result<()> {
        if crate::utils::is_dry_run() {
            println!("[Dry Run] git merge --continue");
            return Ok(());
        }
        let repo = self.repo.borrow();
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("No workdir found"))?;
        use std::io::IsTerminal;
        let is_interactive = std::io::stdin().is_terminal();
        let mut cmd = std::process::Command::new("git");
        cmd.args(["merge", "--continue"]).current_dir(workdir);
        if !is_interactive {
            cmd.env("GIT_EDITOR", "true");
        }
        let status = cmd.status()?;
        if !status.success() {
            bail!("merge continue failed");
        }
        Ok(())
    }

    /// Abort a merge in progress, returning to the pre-merge state.
    pub fn merge_abort(&self) -> Result<()> {
        if crate::utils::is_dry_run() {
            println!("[Dry Run] git merge --abort");
            return Ok(());
        }
        let repo = self.repo.borrow();
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("No workdir found"))?;
        let output = std::process::Command::new("git")
            .args(["merge", "--abort"])
            .current_dir(workdir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("merge abort failed: {}", stderr.trim());
        }
        Ok(())
    }

    /// Continue a cherry-pick after conflict resolution.
    pub fn cherrypick_continue(&self) -> Result<()> {
        if crate::utils::is_dry_run() {
            println!("[Dry Run] git cherry-pick --continue");
            return Ok(());
        }
        let repo = self.repo.borrow();
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("No workdir found"))?;
        use std::io::IsTerminal;
        let is_interactive = std::io::stdin().is_terminal();
        let mut cmd = std::process::Command::new("git");
        cmd.args(["cherry-pick", "--continue"]).current_dir(workdir);
        if !is_interactive {
            cmd.env("GIT_EDITOR", "true");
        }
        let status = cmd.status()?;
        if !status.success() {
            bail!("cherry-pick continue failed");
        }
        Ok(())
    }

    /// Abort a cherry-pick in progress, returning to the pre-cherry-pick state.
    pub fn cherrypick_abort(&self) -> Result<()> {
        if crate::utils::is_dry_run() {
            println!("[Dry Run] git cherry-pick --abort");
            return Ok(());
        }
        let repo = self.repo.borrow();
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("No workdir found"))?;
        let output = std::process::Command::new("git")
            .args(["cherry-pick", "--abort"])
            .current_dir(workdir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("cherry-pick abort failed: {}", stderr.trim());
        }
        Ok(())
    }

    pub fn set_branch_config(&self, branch: &str, key: &str, value: &str) -> Result<()> {
        let repo = self.repo.borrow();
        let mut config = repo.config()?;
        config.set_str(&format!("branch.{}.{}", branch, key), value)?;
        Ok(())
    }

    pub fn get_branch_config(&self, branch: &str, key: &str) -> Result<Option<String>> {
        let repo = self.repo.borrow();
        let config = repo.config()?;
        match config.get_string(&format!("branch.{}.{}", branch, key)) {
            Ok(val) => Ok(Some(val)),
            Err(e) => {
                if e.code() == git2::ErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    pub fn remove_branch_config(&self, branch: &str, key: &str) -> Result<()> {
        let repo = self.repo.borrow();
        let mut config = repo.config()?;
        let full_key = format!("branch.{}.{}", branch, key);
        match config.remove(&full_key) {
            Ok(_) => Ok(()),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Save state for generalize auto-cleanup
    pub fn save_generalize_state(&self, customer_branch: &str, commits: &[String]) -> Result<()> {
        let repo = self.repo.borrow();
        let git_dir = repo.path();
        std::fs::write(git_dir.join("GITFLOW_GENERALIZE_CUSTOMER"), customer_branch)?;
        std::fs::write(git_dir.join("GITFLOW_GENERALIZE_PICKS"), commits.join("\n"))?;
        Ok(())
    }

    /// Load state for generalize auto-cleanup
    pub fn load_generalize_state(&self) -> Result<Option<(String, Vec<String>)>> {
        if let Ok(current_branch) = self.current_branch() {
            if let Ok(Some(cust)) =
                self.get_branch_config(&current_branch, "gitflow-general-from-customer")
            {
                if let Ok(Some(picks_str)) =
                    self.get_branch_config(&current_branch, "gitflow-general-picks")
                {
                    let commits = picks_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    return Ok(Some((cust, commits)));
                }
            }
        }

        let repo = self.repo.borrow();
        let git_dir = repo.path();
        let customer_path = git_dir.join("GITFLOW_GENERALIZE_CUSTOMER");
        let picks_path = git_dir.join("GITFLOW_GENERALIZE_PICKS");
        if !customer_path.exists() || !picks_path.exists() {
            return Ok(None);
        }
        let customer = std::fs::read_to_string(customer_path)?.trim().to_string();
        let picks = std::fs::read_to_string(picks_path)?;
        let commits: Vec<String> = picks
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(Some((customer, commits)))
    }

    /// Clear generalize state
    pub fn clear_generalize_state(&self) -> Result<()> {
        if let Ok(current_branch) = self.current_branch() {
            let _ = self.remove_branch_config(&current_branch, "gitflow-general-from-customer");
            let _ = self.remove_branch_config(&current_branch, "gitflow-general-picks");
        }
        let repo = self.repo.borrow();
        let git_dir = repo.path();
        let _ = std::fs::remove_file(git_dir.join("GITFLOW_GENERALIZE_CUSTOMER"));
        let _ = std::fs::remove_file(git_dir.join("GITFLOW_GENERALIZE_PICKS"));
        Ok(())
    }
}

/// Build credential callbacks for network operations (push/fetch).
fn network_callbacks<'a>() -> RemoteCallbacks<'a> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, allowed_types| {
        if allowed_types.contains(git2::CredentialType::SSH_KEY) {
            let user = username_from_url.unwrap_or("git");
            if let Ok(cred) = Cred::ssh_key_from_agent(user) {
                return Ok(cred);
            }
            let home = std::env::var("HOME").unwrap_or_default();
            let key_path = Path::new(&home).join(".ssh/id_rsa");
            if key_path.exists() {
                if let Ok(cred) = Cred::ssh_key(user, None, &key_path, None) {
                    return Ok(cred);
                }
            }
        }
        Cred::default()
    });
    callbacks
}
