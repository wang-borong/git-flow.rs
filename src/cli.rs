use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[clap(name = "git-flow", version)]
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
    /// start a task
    Start {
        /// input full branch name if no branch type input
        branch_name: String,
        branch_type: Option<String>,
        /// fetch source branch before creating
        #[arg(long)]
        fetch: bool,
    },
    /// finish a task
    Finish {
        /// input full branch name if no branch type input
        branch_name: String,
        branch_type: Option<String>,
        /// keep branch after finish (don't delete)
        #[arg(long)]
        keep: bool,
        /// create a tag after finish
        #[arg(long)]
        tag: bool,
        /// use squash merge strategy
        #[arg(long)]
        squash: bool,
        /// push target branches to remote after finish
        #[arg(long)]
        push: bool,
        /// fetch source branch before finish
        #[arg(long)]
        fetch: bool,
    },
    /// drop a task
    Drop {
        /// input full branch name if no branch type input
        branch_name: String,
        branch_type: Option<String>,
    },
    /// track a task
    Track {
        /// input full branch name if no branch type input
        branch_name: String,
        branch_type: Option<String>,
    },
    /// sync branches
    Sync {
        target: SyncTarget,
        /// default is increment
        strategy: Option<SyncStrategy>,
    },
    /// list avaliable branch types
    List,
    /// check config
    Check { file_path: PathBuf },
    /// generate shell completion
    Complete {
        /// target shell
        #[arg(value_enum)]
        shell: Shell,
    },
    /// initialize git-flow config interactively
    Init,
    /// publish current branch to remote
    Publish {
        /// branch name (defaults to current branch)
        branch_name: Option<String>,
        branch_type: Option<String>,
    },
    /// rebase current branch onto its source branch
    Rebase {
        /// branch name (defaults to current branch)
        branch_name: Option<String>,
        branch_type: Option<String>,
    },
    /// continue after resolving conflicts
    Continue,
    /// abort current operation and restore previous state
    Abort,
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
