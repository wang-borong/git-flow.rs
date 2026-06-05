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
        let msg = custom_msg
            .map(|m| m.to_string())
            .unwrap_or_else(|| format!("Merge branch '{}'", source_branch));
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
        let source_ref = repo.find_branch(source_branch, git2::BranchType::Local)?;
        let source_oid = source_ref.get().target().unwrap();
        let source_commit = repo.find_commit(source_oid)?;
        let source_tree = source_commit.tree()?;

        let head_commit = repo.head()?.peel_to_commit()?;
        let head_tree = head_commit.tree()?;

        let ancestor = repo.merge_base(head_commit.id(), source_oid)?;
        let ancestor_commit = repo.find_commit(ancestor)?;
        let ancestor_tree = ancestor_commit.tree()?;

        let merge_opts = git2::MergeOptions::new();
        let mut index =
            repo.merge_trees(&ancestor_tree, &head_tree, &source_tree, Some(&merge_opts))?;

        if index.has_conflicts() {
            bail!("squash merge conflict detected, resolve manually");
        }

        let result_tree = repo.find_tree(index.write_tree_to(&repo)?)?;
        let sig = repo.signature()?;
        let msg = custom_msg
            .map(|m| m.to_string())
            .unwrap_or_else(|| format!("Squash merge branch '{}'", source_branch));
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &msg,
            &result_tree,
            &[&head_commit],
        )?;

        // Update active index to the result tree
        let mut repo_index = repo.index()?;
        repo_index.read_tree(&result_tree)?;
        repo_index.write()?;

        // Checkout the result tree to update the worktree
        let mut checkout_opts = CheckoutBuilder::new();
        checkout_opts.force();
        repo.checkout_head(Some(&mut checkout_opts))?;

        repo.cleanup_state()?;
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
        let output = std::process::Command::new("git")
            .args(["rebase", "--continue"])
            .current_dir(workdir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("rebase continue failed: {}", stderr.trim());
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
        let output = std::process::Command::new("git")
            .args(["merge", "--continue"])
            .current_dir(workdir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("merge continue failed: {}", stderr.trim());
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
        let output = std::process::Command::new("git")
            .args(["cherry-pick", "--continue"])
            .current_dir(workdir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("cherry-pick continue failed: {}", stderr.trim());
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
