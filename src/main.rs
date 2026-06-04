use clap::CommandFactory;
use clap::Parser;
use cli::{Args, Command};
use echo::Echo;
use utils::{env_valid, get_branch_type_name};

mod cli;
mod command;
mod config;
mod echo;
mod git;
mod rules;
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
        Command::Customer { action } => {
            if !env_valid() {
                return;
            }

            let (main_branch, customer_prefix, remote) =
                read_yqm_branch_config(args.config.clone());
            let remote_ref = remote.as_deref();

            match action {
                cli::CustomerAction::Create {
                    customer_name,
                    push,
                } => {
                    command::customer::create_customer(
                        customer_name,
                        &main_branch,
                        if *push { remote_ref } else { None },
                    );
                }
                cli::CustomerAction::Sync {
                    customer_name,
                    push,
                } => {
                    if customer_name == "all" {
                        command::customer::sync_all_customers(
                            &main_branch,
                            *push,
                            remote_ref,
                            &customer_prefix,
                        );
                    } else {
                        command::customer::sync_customer(
                            customer_name,
                            &main_branch,
                            *push,
                            remote_ref,
                        );
                    }
                }
                cli::CustomerAction::List => {
                    command::customer::list_customers(&main_branch, &customer_prefix);
                }
            }
        }

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
            customer,
        } => {
            if !env_valid() {
                return;
            }

            match get_branch_type_name(branch_name.clone(), branch_type.clone(), args.config) {
                Err(err) => Echo::error(err.to_string()),
                Ok((branch_name, mut branch_type)) => {
                    // Override from/to if --customer is specified
                    if let Some(customer_name) = customer {
                        let customer_branch = format!("customer/{}", customer_name);
                        branch_type.from = customer_branch.clone();
                        // Override target to customer branch
                        for to in &mut branch_type.to {
                            to.name = customer_branch.clone();
                        }
                    }
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
            customer,
        } => {
            if !env_valid() {
                return;
            }

            match get_branch_type_name(branch_name.clone(), branch_type.clone(), args.config) {
                Err(err) => Echo::error(err.to_string()),
                Ok((branch_name, mut branch_type)) => {
                    // Override from/to if --customer is specified
                    if let Some(customer_name) = customer {
                        let customer_branch = format!("customer/{}", customer_name);
                        branch_type.from = customer_branch.clone();
                        for to in &mut branch_type.to {
                            to.name = customer_branch.clone();
                        }
                    }
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

/// Read yqm config to extract main branch name, customer prefix, and remote.
/// Falls back to defaults if not a yqm config.
fn read_yqm_branch_config(
    config_path: Option<std::path::PathBuf>,
) -> (String, String, Option<String>) {
    let default_main = "main".to_string();
    let default_prefix = "customer/".to_string();

    // Try to find and read the config file
    let config_text = if let Some(path) = config_path {
        std::fs::read_to_string(path).ok()
    } else {
        // Try local then global config
        let paths = config::path::get_config_path_list().unwrap_or_default();
        paths.iter().find_map(|p| std::fs::read_to_string(p).ok())
    };

    let text = match config_text {
        Some(t) => t,
        None => return (default_main, default_prefix, None),
    };

    if !config::yqm::is_yqm_format(&text) {
        return (default_main, default_prefix, None);
    }

    match toml::from_str::<config::yqm::YqmConfig>(&text) {
        Ok(yqm) => {
            let remote = yqm.hooks.post_start.as_ref().map(|_| {
                // Try to detect remote from the config
                "origin".to_string()
            });
            (yqm.branches.main, yqm.branches.customer.prefix, remote)
        }
        Err(_) => (default_main, default_prefix, None),
    }
}
