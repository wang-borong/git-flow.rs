use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub branch_types: Vec<BranchType>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BranchType {
    pub name: String,
    pub create: String,
    pub from: String,
    pub to: Vec<TargetBranch>,
    /// Default remote name for publish/push operations (e.g. "origin")
    pub remote: Option<String>,
    /// Tag name pattern for finish --tag (e.g. "v{NAME}")
    pub tag_pattern: Option<String>,
    pub before_start: Option<Command>,
    pub after_start: Option<Command>,
    pub before_finish: Option<Command>,
    pub after_finish: Option<Command>,
    pub before_drop: Option<Command>,
    pub after_drop: Option<Command>,
    pub before_publish: Option<Command>,
    pub after_publish: Option<Command>,
    pub before_rebase: Option<Command>,
    pub after_rebase: Option<Command>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TargetBranch {
    pub name: String,
    pub strategy: Strategy,
    /// Auto-push to remote after merge into this target branch
    pub push: Option<bool>,
    /// Auto-tag after merge into this target branch
    pub tag: Option<bool>,
}

#[derive(Debug, Deserialize, Clone)]
pub enum Strategy {
    #[serde(rename = "merge")]
    Merge,
    #[serde(rename = "rebase")]
    Rebase,
    #[serde(rename = "cherry-pick")]
    CherryPick,
    #[serde(rename = "squash")]
    Squash,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Command {
    pub command: String,
    pub args: Vec<String>,
}

pub const BRANCH_NAME_PLACEHOLDER: &str = "{NAME}";
