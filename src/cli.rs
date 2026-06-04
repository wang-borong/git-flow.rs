use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[clap(name = "gitflow", version)]
pub struct Args {
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Show what would be done without executing
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Show detailed git operations
    #[arg(long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage feature branches
    Feature {
        #[command(subcommand)]
        action: FeatureAction,
    },
    /// Manage release branches
    Release {
        #[command(subcommand)]
        action: ReleaseAction,
    },
    /// Manage hotfix branches
    Hotfix {
        #[command(subcommand)]
        action: HotfixAction,
    },
    /// Manage customer branches (long-lived)
    Custom {
        #[command(subcommand)]
        action: CustomAction,
    },
    /// Manage generalize branches (定制转通用)
    General {
        #[command(subcommand)]
        action: GeneralAction,
    },
    /// Finish current branch (shorthand)
    Finish {
        /// keep branch after finish (don't delete)
        #[arg(long)]
        keep: bool,
        /// create a tag after finish (optional: specify custom tag name)
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        tag: Option<String>,
        /// use squash merge strategy
        #[arg(long)]
        squash: bool,
        /// push target branches to remote after finish
        #[arg(long)]
        push: bool,
        /// fetch source branch before finish
        #[arg(long)]
        fetch: bool,
        /// merge to a customer branch instead of main
        #[arg(short, long)]
        customer: Option<String>,
        /// bump version: "minor" or "patch"
        #[arg(long)]
        bump: Option<String>,
        /// sign the tag
        #[arg(long)]
        sign: bool,
        /// continue after resolving conflicts
        #[arg(long)]
        r#continue: bool,
        /// force rebase strategy
        #[arg(long)]
        rebase: bool,
        /// squash message
        #[arg(long)]
        squash_message: Option<String>,
        /// custom merge message
        #[arg(long)]
        merge_message: Option<String>,
        /// update message (for release or sync)
        #[arg(long)]
        update_message: Option<String>,
        /// skip verification hooks
        #[arg(long)]
        no_verify: bool,
    },
    /// Update current branch (shorthand)
    Update {
        /// update with rebase regardless of config
        #[arg(long)]
        rebase: bool,
    },
    /// Delete current branch (shorthand)
    Delete {
        /// delete with remote cleanup
        #[arg(long)]
        remote: bool,
        /// force delete branch with unmerged changes
        #[arg(long)]
        force: bool,
    },
    /// Rename current branch (shorthand)
    Rename {
        /// new name of the branch
        new_name: String,
    },
    /// Publish current branch (shorthand)
    Publish {
        /// branch name (optional, defaults to current)
        name: Option<String>,
    },
    /// continue after resolving conflicts
    Continue,
    /// abort current operation and restore previous state
    Abort,
    /// check config
    Check { file_path: PathBuf },
    /// generate shell completion
    Complete {
        /// target shell
        #[arg(value_enum)]
        shell: Shell,
    },
    /// initialize gitflow config interactively
    Init,
    /// sync branches (deprecated top level)
    Sync {
        target: SyncTarget,
        /// default is increment
        strategy: Option<SyncStrategy>,
    },
    /// list available branch types
    List,
    /// display a comprehensive overview of repository status
    Overview,
}

#[derive(Debug, Subcommand)]
pub enum FeatureAction {
    /// Start a new feature
    Start {
        name: String,
        base: Option<String>,
        #[arg(long)]
        fetch: bool,
        #[arg(short, long)]
        customer: Option<String>,
    },
    /// Finish a feature branch
    Finish {
        name: Option<String>,
        #[arg(short, long)]
        customer: Option<String>,
        #[arg(long)]
        keep: bool,
        /// create a tag after finish (optional: specify custom tag name)
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        tag: Option<String>,
        #[arg(long)]
        squash: bool,
        #[arg(long)]
        push: bool,
        #[arg(long)]
        fetch: bool,
        #[arg(long)]
        bump: Option<String>,
        #[arg(long)]
        sign: bool,
        #[arg(long)]
        r#continue: bool,
        #[arg(long)]
        rebase: bool,
        #[arg(long)]
        squash_message: Option<String>,
        #[arg(long)]
        merge_message: Option<String>,
        #[arg(long)]
        no_verify: bool,
    },
    /// Update a feature branch
    Update {
        name: Option<String>,
        #[arg(long)]
        rebase: bool,
    },
    /// Delete a feature branch
    Delete {
        name: Option<String>,
        #[arg(long)]
        remote: bool,
        #[arg(long)]
        force: bool,
    },
    /// Rename a feature branch
    Rename {
        old_name: String,
        new_name: Option<String>,
    },
    /// Checkout a feature branch
    Checkout { name: String },
    /// Track a feature branch
    Track { name: String },
    /// List feature branches
    List { pattern: Option<String> },
    /// Publish a feature branch
    Publish { name: Option<String> },
}

#[derive(Debug, Subcommand)]
pub enum ReleaseAction {
    /// Start a new release
    Start {
        name: String,
        base: Option<String>,
        #[arg(long)]
        fetch: bool,
        #[arg(short, long)]
        customer: Option<String>,
    },
    /// Finish a release branch
    Finish {
        name: Option<String>,
        #[arg(short, long)]
        customer: Option<String>,
        #[arg(long)]
        keep: bool,
        /// create a tag after finish (optional: specify custom tag name)
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        tag: Option<String>,
        #[arg(long)]
        squash: bool,
        #[arg(long)]
        push: bool,
        #[arg(long)]
        fetch: bool,
        #[arg(long)]
        bump: Option<String>,
        #[arg(long)]
        sign: bool,
        #[arg(long)]
        r#continue: bool,
        #[arg(long)]
        rebase: bool,
        #[arg(long)]
        squash_message: Option<String>,
        #[arg(long)]
        merge_message: Option<String>,
        #[arg(long)]
        update_message: Option<String>,
        #[arg(long)]
        no_verify: bool,
    },
    /// Update a release branch
    Update {
        name: Option<String>,
        #[arg(long)]
        rebase: bool,
    },
    /// Delete a release branch
    Delete {
        name: Option<String>,
        #[arg(long)]
        remote: bool,
        #[arg(long)]
        force: bool,
    },
    /// Rename a release branch
    Rename {
        old_name: String,
        new_name: Option<String>,
    },
    /// Checkout a release branch
    Checkout { name: String },
    /// Track a release branch
    Track { name: String },
    /// List release branches
    List { pattern: Option<String> },
    /// Publish a release branch
    Publish { name: Option<String> },
}

#[derive(Debug, Subcommand)]
pub enum HotfixAction {
    /// Start a new hotfix
    Start {
        name: String,
        base: Option<String>,
        #[arg(long)]
        fetch: bool,
        #[arg(short, long)]
        customer: Option<String>,
    },
    /// Finish a hotfix branch
    Finish {
        name: Option<String>,
        #[arg(short, long)]
        customer: Option<String>,
        #[arg(long)]
        keep: bool,
        /// create a tag after finish (optional: specify custom tag name)
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        tag: Option<String>,
        #[arg(long)]
        squash: bool,
        #[arg(long)]
        push: bool,
        #[arg(long)]
        fetch: bool,
        #[arg(long)]
        bump: Option<String>,
        #[arg(long)]
        sign: bool,
        #[arg(long)]
        r#continue: bool,
        #[arg(long)]
        rebase: bool,
        #[arg(long)]
        squash_message: Option<String>,
        #[arg(long)]
        merge_message: Option<String>,
        #[arg(long)]
        no_verify: bool,
    },
    /// Update a hotfix branch
    Update {
        name: Option<String>,
        #[arg(long)]
        rebase: bool,
    },
    /// Delete a hotfix branch
    Delete {
        name: Option<String>,
        #[arg(long)]
        remote: bool,
        #[arg(long)]
        force: bool,
    },
    /// Rename a hotfix branch
    Rename {
        old_name: String,
        new_name: Option<String>,
    },
    /// Checkout a hotfix branch
    Checkout { name: String },
    /// Track a hotfix branch
    Track { name: String },
    /// List hotfix branches
    List { pattern: Option<String> },
    /// Publish a hotfix branch
    Publish { name: Option<String> },
}

#[derive(Debug, Subcommand)]
pub enum CustomAction {
    /// Start (create) a customer branch
    Start {
        name: String,
        #[arg(long)]
        push: bool,
        #[arg(long)]
        fetch: bool,
    },
    /// Finish a customer branch
    Finish {
        name: String,
        /// Force finish (required for customer branches as they are long-lived)
        #[arg(long)]
        force: bool,
    },
    /// Sync main changes into customer branch
    Sync {
        /// Customer name or "all"
        customer_name: String,
        #[arg(long)]
        push: bool,
    },
    /// Checkout a customer branch
    Checkout { name: String },
    /// Delete a customer branch
    Delete {
        name: String,
        #[arg(long)]
        remote: bool,
        #[arg(long)]
        force: bool,
    },
    /// Rename a customer branch
    Rename {
        old_name: String,
        new_name: Option<String>,
    },
    /// List all customer branches
    List,
}

#[derive(Debug, Subcommand)]
pub enum GeneralAction {
    /// Start a new general (定制转通用) branch
    Start {
        name: String,
        /// Source customer name
        #[arg(short, long)]
        customer: String,
        /// Target merge branch (e.g. main or other-customer)
        #[arg(short, long)]
        to: String,
        /// Commits to cherry-pick
        #[arg(long)]
        pick: String,
        #[arg(long)]
        fetch: bool,
    },
    /// Finish a general branch
    Finish {
        name: Option<String>,
        #[arg(long)]
        keep: bool,
        #[arg(long)]
        no_verify: bool,
        #[arg(long)]
        r#continue: bool,
    },
    /// Update a general branch
    Update {
        name: Option<String>,
        #[arg(long)]
        rebase: bool,
    },
    /// Delete a general branch
    Delete {
        name: Option<String>,
        #[arg(long)]
        remote: bool,
        #[arg(long)]
        force: bool,
    },
    /// Rename a general branch
    Rename {
        old_name: String,
        new_name: Option<String>,
    },
    /// Checkout a general branch
    Checkout { name: String },
    /// List general branches
    List { pattern: Option<String> },
    /// Publish a general branch
    Publish { name: Option<String> },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum SyncTarget {
    Local,
    Remote,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum SyncStrategy {
    Override,
    Increment,
}
