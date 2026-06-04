use anyhow::Result;
use git2::{BranchType, FetchOptions, PushOptions};

use super::{network_callbacks, Git};

// # delete
impl Git {
    pub fn del_local_branch(&self, target_branch: &str) -> Result<()> {
        let repo = self.repo.borrow();
        let mut branch = repo.find_branch(target_branch, BranchType::Local)?;
        branch.delete()?;
        Ok(())
    }

    pub fn del_remote_branch(&self, target_repo: &str, target_branch: &str) -> Result<()> {
        let repo = self.repo.borrow();
        let mut remote = repo.find_remote(target_repo)?;
        let refspec = format!(":refs/heads/{}", target_branch);
        remote.push(
            &[&refspec],
            Some(PushOptions::new().remote_callbacks(network_callbacks())),
        )?;
        Ok(())
    }
}

// # create
impl Git {
    pub fn create_local_branch(&self, source_branch: &str, target_branch: &str) -> Result<()> {
        let repo = self.repo.borrow();
        let source_ref = repo.find_branch(source_branch, BranchType::Local)?;
        let commit = source_ref.get().peel_to_commit()?;
        repo.branch(target_branch, &commit, false)?;
        Ok(())
    }

    /// Create remote branch from local branch
    pub fn create_remote_branch(
        &self,
        repo_name: &str,
        local_branch: &str,
        remote_branch: &str,
    ) -> Result<()> {
        let repo = self.repo.borrow();
        let mut remote = repo.find_remote(repo_name)?;
        let refspec = format!("refs/heads/{}:refs/heads/{}", local_branch, remote_branch);
        remote.push(
            &[&refspec],
            Some(PushOptions::new().remote_callbacks(network_callbacks())),
        )?;
        Ok(())
    }

    /// Push a local branch to a remote.
    pub fn push_branch(
        &self,
        remote_name: &str,
        local_branch: &str,
        remote_branch: &str,
    ) -> Result<()> {
        let repo = self.repo.borrow();
        let mut remote = repo.find_remote(remote_name)?;
        let refspec = format!("refs/heads/{}:refs/heads/{}", local_branch, remote_branch);
        remote.push(
            &[&refspec],
            Some(PushOptions::new().remote_callbacks(network_callbacks())),
        )?;
        Ok(())
    }
}

// # get
impl Git {
    pub fn fetch_remote_data(&self) -> Result<()> {
        let repo = self.repo.borrow();
        let remotes = repo.remotes()?;
        for entry in remotes.iter() {
            if let Some(remote_name) = entry? {
                let mut remote = repo.find_remote(remote_name)?;
                let mut fetch_opts = FetchOptions::new();
                fetch_opts.remote_callbacks(network_callbacks());
                remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None)?;
            }
        }
        Ok(())
    }

    /// Fetch a specific remote.
    pub fn fetch_remote(&self, remote_name: &str) -> Result<()> {
        let repo = self.repo.borrow();
        let mut remote = repo.find_remote(remote_name)?;
        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(network_callbacks());
        remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None)?;
        Ok(())
    }

    pub fn get_local_branches(&self) -> Result<Vec<String>> {
        let repo = self.repo.borrow();
        let branches = repo.branches(Some(BranchType::Local))?;
        let mut names = Vec::new();
        for branch in branches {
            let (branch, _) = branch?;
            if let Some(name) = branch.name()? {
                names.push(name.to_string());
            }
        }
        Ok(names)
    }

    pub fn get_remote_branches(&self, repo_name: &str) -> Result<Vec<String>> {
        let repo = self.repo.borrow();
        let branches = repo.branches(Some(BranchType::Remote))?;
        let prefix = format!("{}/", repo_name);
        let mut names = Vec::new();
        for branch in branches {
            let (branch, _) = branch?;
            if let Some(name) = branch.name()? {
                if let Some(branch_name) = name.strip_prefix(&prefix) {
                    names.push(branch_name.to_string());
                }
            }
        }
        Ok(names)
    }
}

// # tag
impl Git {
    /// Create an annotated tag at the current HEAD.
    pub fn create_tag(&self, tag_name: &str, message: &str) -> Result<()> {
        let repo = self.repo.borrow();
        let head = repo.head()?.peel_to_commit()?;
        let sig = repo.signature()?;
        repo.tag(tag_name, head.as_object(), &sig, message, false)?;
        Ok(())
    }
}
