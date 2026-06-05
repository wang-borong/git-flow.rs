use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::command::finish::FinishOptions;
use crate::config::definition::{BranchType, TargetBranch};
use crate::git::Git;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum GitflowState {
    Finish {
        branch_name: String,
        branch_type: BranchType,
        opts: FinishOptions,
        target_branches: Vec<TargetBranch>,
        remaining_targets: Vec<TargetBranch>,
    },
    Start {
        branch_name: String,
        branch_type: BranchType,
    },
    /// Saved when `gitflow custom sync` hits a merge/rebase conflict mid-sync
    Sync {
        /// The customer branch currently being synced (e.g. "customer/acme")
        customer_branch: String,
        /// The main/source branch being synced from (e.g. "main")
        main_branch: String,
        /// Whether the sync was using rebase (true) or merge (false)
        rebase: bool,
        /// Whether to push after resolution
        push: bool,
        /// Remote name for push (if any)
        remote: Option<String>,
        /// The branch the user was on before sync started (for workspace restoration)
        original_branch: String,
    },
}

impl GitflowState {
    fn file_path(git: &Git) -> PathBuf {
        git.git_dir().join("GITFLOW_STATE")
    }

    pub fn save(&self, git: &Git) -> Result<()> {
        let content = toml::to_string(self).context("Failed to serialize GitflowState")?;
        let path = Self::file_path(git);
        fs::write(path, content).context("Failed to write GITFLOW_STATE file")?;
        Ok(())
    }

    pub fn load(git: &Git) -> Result<Option<Self>> {
        let path = Self::file_path(git);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(path).context("Failed to read GITFLOW_STATE file")?;
        let state = toml::from_str(&content).context("Failed to deserialize GitflowState")?;
        Ok(Some(state))
    }

    pub fn clear(git: &Git) -> Result<()> {
        let path = Self::file_path(git);
        if path.exists() {
            fs::remove_file(path).context("Failed to remove GITFLOW_STATE file")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::definition::Strategy;

    #[test]
    fn test_gitflow_state_serialization() {
        let branch_type = BranchType {
            name: "feature".to_string(),
            create: "feature/".to_string(),
            from: "main".to_string(),
            to: vec![TargetBranch {
                name: "main".to_string(),
                strategy: Strategy::Merge,
                push: Some(true),
                tag: Some(true),
            }],
            remote: Some("origin".to_string()),
            tag_pattern: Some("v{NAME}".to_string()),
            before_start: None,
            after_start: None,
            before_finish: None,
            after_finish: None,
            before_drop: None,
            after_drop: None,
            before_publish: None,
            after_publish: None,
            before_rebase: None,
            after_rebase: None,
        };

        let opts = FinishOptions {
            keep: false,
            tag: None,
            squash: false,
            push: true,
            fetch: false,
            bump: None,
            squash_message: None,
            merge_message: None,
            update_message: None,
            no_verify: false,
            sign: false,
            customer: None,
            cleanup_customer: false,
        };

        let state = GitflowState::Finish {
            branch_name: "feature/my-feat".to_string(),
            branch_type: branch_type.clone(),
            opts,
            target_branches: branch_type.to.clone(),
            remaining_targets: branch_type.to.clone(),
        };

        let serialized = toml::to_string(&state).unwrap();
        let deserialized: GitflowState = toml::from_str(&serialized).unwrap();

        match deserialized {
            GitflowState::Finish { branch_name, .. } => {
                assert_eq!(branch_name, "feature/my-feat");
            }
            _ => panic!("Expected GitflowState::Finish"),
        }
    }
}
