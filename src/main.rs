use clap::Parser;
use clap::CommandFactory;
use cli::{Args, Command};
use echo::Echo;
use utils::{env_valid, get_branch_type_name};

mod cli;
mod command;
mod config;
mod echo;
mod git;
mod utils;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match &args.command {
        // -- commands that don't require env_valid --
        Command::Complete { shell } => {
            let mut cmd = Args::command();
            clap_complete::generate(*shell, &mut cmd, "git-flow", &mut std::io::stdout());
        }
        Command::List => command::list::list_branch_types(args.config),
        Command::Check { file_path } => command::check::check_config(file_path.clone()),
        Command::Init => command::init::init_config(),
        Command::Continue => command::continue_cmd::continue_operation(),
        Command::Abort => command::abort_cmd::abort_operation(),

        // -- commands that require env_valid --
        Command::Sync { target, strategy } => {
            if !env_valid() {
                return;
            }

            command::sync::sync_repo_branches(
                target.clone(),
                strategy.clone().unwrap_or(cli::SyncStrategy::Increment),
            );
        }
        Command::Start {
            branch_name,
            branch_type,
            fetch,
        } => {
            if !env_valid() {
                return;
            }

            match get_branch_type_name(branch_name.clone(), branch_type.clone(), args.config) {
                Err(err) => Echo::error(err.to_string()),
                Ok((branch_name, branch_type)) => {
                    command::start::start_task(branch_name, branch_type, *fetch);
                }
            }
        }
        Command::Finish {
            branch_name,
            branch_type,
            keep,
            tag,
            squash,
            push,
            fetch,
        } => {
            if !env_valid() {
                return;
            }

            match get_branch_type_name(branch_name.clone(), branch_type.clone(), args.config) {
                Err(err) => Echo::error(err.to_string()),
                Ok((branch_name, branch_type)) => {
                    let opts = command::finish::FinishOptions {
                        keep: *keep,
                        tag: *tag,
                        squash: *squash,
                        push: *push,
                        fetch: *fetch,
                    };
                    command::finish::finish_task(branch_name, branch_type, opts);
                }
            }
        }
        Command::Drop {
            branch_name,
            branch_type,
        } => {
            if !env_valid() {
                return;
            }

            match get_branch_type_name(branch_name.clone(), branch_type.clone(), args.config) {
                Err(err) => Echo::error(err.to_string()),
                Ok((branch_name, branch_type)) => {
                    command::drop::drop_task(branch_name, branch_type);
                }
            }
        }
        Command::Track {
            branch_name,
            branch_type,
        } => {
            if !env_valid() {
                return;
            }

            match get_branch_type_name(branch_name.clone(), branch_type.clone(), args.config) {
                Err(err) => Echo::error(err.to_string()),
                Ok((branch_name, branch_type)) => {
                    command::track::track_task(branch_name, branch_type);
                }
            }
        }
        Command::Publish {
            branch_name,
            branch_type,
        } => {
            if !env_valid() {
                return;
            }

            let resolved_name = match branch_name {
                Some(name) => name.clone(),
                None => match git::Git::open().and_then(|g| g.current_branch()) {
                    Ok(name) => name,
                    Err(err) => {
                        Echo::error(err.to_string());
                        return;
                    }
                },
            };

            match get_branch_type_name(resolved_name, branch_type.clone(), args.config) {
                Err(err) => Echo::error(err.to_string()),
                Ok((branch_name, branch_type)) => {
                    command::publish::publish_branch(branch_name, branch_type);
                }
            }
        }
        Command::Rebase {
            branch_name,
            branch_type,
        } => {
            if !env_valid() {
                return;
            }

            let resolved_name = match branch_name {
                Some(name) => name.clone(),
                None => match git::Git::open().and_then(|g| g.current_branch()) {
                    Ok(name) => name,
                    Err(err) => {
                        Echo::error(err.to_string());
                        return;
                    }
                },
            };

            match get_branch_type_name(resolved_name, branch_type.clone(), args.config) {
                Err(err) => Echo::error(err.to_string()),
                Ok((branch_name, branch_type)) => {
                    command::rebase_cmd::rebase_branch(branch_name, branch_type);
                }
            }
        }
    }
}
