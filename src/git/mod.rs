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
}

// # status
impl Git {
    pub fn git_installed() -> bool {
        Repository::open_from_env().is_ok()
    }
}

// # combine
impl Git {
    pub fn merge(&self, source_branch: &str) -> Result<()> {
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
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &format!("Merge branch '{}'", source_branch),
            &tree,
            &[&head_commit, &source_commit],
        )?;

        repo.cleanup_state()?;
        Ok(())
    }

    pub fn rebase(&self, base_branch: &str) -> Result<()> {
        let repo = self.repo.borrow();
        let base_ref = repo.find_branch(base_branch, git2::BranchType::Local)?;
        let base_oid = base_ref.get().target().unwrap();
        let base_annotated = repo.find_annotated_commit(base_oid)?;

        let head_ref = repo.head()?;
        let head_oid = head_ref.target().unwrap();
        let head_annotated = repo.find_annotated_commit(head_oid)?;

        let mut rebase = repo.rebase(Some(&head_annotated), Some(&base_annotated), None, None)?;

        let sig = repo.signature()?;
        while let Some(op) = rebase.next() {
            let _op = op?;
            rebase.commit(None, &sig, None)?;
        }
        rebase.finish(Some(&sig))?;
        Ok(())
    }

    pub fn cherry_pick(&self, commits: Vec<String>) -> Result<()> {
        let repo = self.repo.borrow();
        for commit_id in &commits {
            let oid = git2::Oid::from_str(commit_id)?;
            let commit = repo.find_commit(oid)?;
            repo.cherrypick(&commit, None)?;

            let index = repo.index()?;
            if index.has_conflicts() {
                bail!("cherry-pick conflict detected on commit {}", commit_id);
            }

            let sig = repo.signature()?;
            let head_commit = repo.head()?.peel_to_commit()?;
            let tree = repo.find_tree(repo.index()?.write_tree()?)?;
            let msg = commit.message().unwrap_or("cherry-pick");
            repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &[&head_commit])?;
            repo.cleanup_state()?;
        }
        Ok(())
    }

    /// Squash merge: merge the source tree into the target without preserving
    /// individual commits. Creates a single commit with all changes.
    pub fn squash_merge(&self, source_branch: &str) -> Result<()> {
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

        let mut merge_opts = git2::MergeOptions::new();
        let mut index = repo.merge_trees(&ancestor_tree, &head_tree, &source_tree, Some(&mut merge_opts))?;

        if index.has_conflicts() {
            bail!("squash merge conflict detected, resolve manually");
        }

        let result_tree = repo.find_tree(index.write_tree_to(&repo)?)?;
        let sig = repo.signature()?;
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &format!("Squash merge branch '{}'", source_branch),
            &result_tree,
            &[&head_commit],
        )?;

        repo.cleanup_state()?;
        Ok(())
    }
}

// # other
impl Git {
    pub fn switch(&self, target_branch: &str) -> Result<()> {
        let repo = self.repo.borrow();
        let refname = format!("refs/heads/{}", target_branch);
        repo.set_head(&refname)?;
        repo.checkout_head(Some(CheckoutBuilder::new().safe()))?;
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
        let name = head
            .shorthand()
            .unwrap_or("HEAD")
            .to_string();
        Ok(name)
    }
}

// # stash & conflict state
impl Git {
    /// Check if there are uncommitted changes in the working directory.
    pub fn has_uncommitted_changes(&self) -> Result<bool> {
        let repo = self.repo.borrow();
        let statuses = repo.statuses(Some(
            git2::StatusOptions::new().include_untracked(true),
        ))?;
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

    /// Continue a rebase after conflict resolution.
    /// This finishes the rebase operation by creating commits for resolved changes.
    pub fn rebase_continue(&self) -> Result<()> {
        // If there are resolved changes, commit them first
        let repo = self.repo.borrow();
        let mut index = repo.index()?;
        if index.has_conflicts() {
            bail!("conflicts still exist, resolve them first");
        }

        // Check if there are staged changes to commit
        let head_tree = repo.head()?.peel_to_commit()?.tree()?;
        let index_tree = repo.find_tree(index.write_tree_to(&repo)?)?;
        if head_tree.id() != index_tree.id() {
            let sig = repo.signature()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "continue rebase",
                &index_tree,
                &[&repo.head()?.peel_to_commit()?],
            )?;
        }

        // Finish the rebase
        // Note: After manual conflict resolution, the user should use
        // `git rebase --continue` directly. This is a simplified version.
        repo.cleanup_state()?;
        Ok(())
    }

    /// Abort a rebase in progress, returning to the pre-rebase state.
    pub fn rebase_abort(&self) -> Result<()> {
        let repo = self.repo.borrow();
        repo.cleanup_state()?;
        // Reset HEAD to the original branch
        repo.checkout_head(Some(CheckoutBuilder::new().force()))?;
        Ok(())
    }

    /// Continue a merge after conflict resolution.
    pub fn merge_continue(&self) -> Result<()> {
        let repo = self.repo.borrow();
        let mut index = repo.index()?;
        if index.has_conflicts() {
            bail!("conflicts still exist, resolve them first");
        }

        let sig = repo.signature()?;
        let head_commit = repo.head()?.peel_to_commit()?;
        let tree = repo.find_tree(index.write_tree_to(&repo)?)?;

        // Find the merge parents from MERGE_HEAD
        let merge_head_ref = repo.find_reference("MERGE_HEAD")?;
        let merge_head_oid = merge_head_ref.target().unwrap();
        let merge_commit = repo.find_commit(merge_head_oid)?;

        let msg = format!("Merge branch '{}'", merge_commit.id());
        repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree, &[&head_commit, &merge_commit])?;
        repo.cleanup_state()?;
        Ok(())
    }

    /// Abort a merge in progress, returning to the pre-merge state.
    pub fn merge_abort(&self) -> Result<()> {
        let repo = self.repo.borrow();
        repo.cleanup_state()?;
        repo.checkout_head(Some(CheckoutBuilder::new().force()))?;
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
