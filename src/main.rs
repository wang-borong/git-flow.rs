use std::path::PathBuf;

use clap::CommandFactory;
use clap::Parser;
use cli::{Args, Command, CustomAction, FeatureAction, GeneralAction, HotfixAction, ReleaseAction};
use config::definition::BranchType;
use config::read::read_config;
use echo::Echo;
use git::Git;
use regex::Regex;
use utils::{env_valid, get_branch_type_name};

mod cli;
mod command;
mod config;
mod echo;
mod git;
mod rules;
mod utils;

fn resolve_start_branch(
    type_name: &str,
    name: &str,
    customer: Option<&str>,
    to: Option<&str>,
    config_path: Option<PathBuf>,
) -> anyhow::Result<(String, BranchType)> {
    let config = read_config(config_path)?;

    let target_type_name = match customer {
        Some(_) => {
            if type_name == "feature" || type_name == "hotfix" {
                format!("customer-{}", type_name)
            } else {
                type_name.to_string()
            }
        }
        None => type_name.to_string(),
    };

    let branch_type = config
        .branch_types
        .iter()
        .find(|b| b.name == target_type_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Branch type '{}' not found in configuration",
                target_type_name
            )
        })?;

    let mut branch_type = branch_type.clone();

    // Resolve branch name using template
    let mut branch_name = branch_type.create.clone();

    // Handle customer replacements
    if let Some(cust) = customer {
        if target_type_name.starts_with("customer-") {
            branch_name = branch_name
                .replace("{{NAME}}", cust)
                .replace("{NAME}", cust);
            branch_type.from = branch_type
                .from
                .replace("{{NAME}}", cust)
                .replace("{NAME}", cust);
            for to_branch in &mut branch_type.to {
                to_branch.name = to_branch
                    .name
                    .replace("{{NAME}}", cust)
                    .replace("{NAME}", cust);
            }
        } else {
            branch_name = branch_name
                .replace("{{CUSTOMER}}", cust)
                .replace("{CUSTOMER}", cust);
            branch_type.from = branch_type
                .from
                .replace("{{CUSTOMER}}", cust)
                .replace("{CUSTOMER}", cust);
            for to_branch in &mut branch_type.to {
                to_branch.name = to_branch
                    .name
                    .replace("{{CUSTOMER}}", cust)
                    .replace("{CUSTOMER}", cust);
            }
        }
    }

    // Replace branch name placeholder
    branch_name = branch_name
        .replace("{{FEATURE}}", name)
        .replace("{{FIX}}", name)
        .replace("{{NAME}}", name)
        .replace("{NAME}", name);

    // If --to option is specified (for general branch), override target branches
    if let Some(to_val) = to {
        let target_branch_name = if to_val == "main" {
            config
                .branch_types
                .iter()
                .find(|b| b.name == "feature")
                .map(|b| b.from.clone())
                .unwrap_or_else(|| to_val.to_string())
        } else {
            format!("customer/{}", to_val)
        };

        branch_type.from = target_branch_name.clone();

        for to_branch in &mut branch_type.to {
            to_branch.name = target_branch_name.clone();
        }
    }

    Ok((branch_name, branch_type))
}

fn resolve_branch_info(
    type_name: &str,
    name_opt: Option<String>,
    customer_opt: Option<String>,
    config_path: Option<PathBuf>,
) -> anyhow::Result<(String, BranchType)> {
    let git = Git::open()?;
    let config = read_config(config_path)?;

    let full_branch_name = match name_opt {
        Some(name) => {
            if name.contains('/') {
                name
            } else {
                let target_type_name = match &customer_opt {
                    Some(_) => {
                        if type_name == "feature" || type_name == "hotfix" {
                            format!("customer-{}", type_name)
                        } else {
                            type_name.to_string()
                        }
                    }
                    None => type_name.to_string(),
                };

                let bt = config
                    .branch_types
                    .iter()
                    .find(|b| b.name == target_type_name)
                    .ok_or_else(|| {
                        anyhow::anyhow!("Branch type '{}' not found", target_type_name)
                    })?;

                let mut template = bt.create.clone();
                if let Some(ref cust) = customer_opt {
                    if target_type_name.starts_with("customer-") {
                        template = template.replace("{{NAME}}", cust).replace("{NAME}", cust);
                    } else {
                        template = template
                            .replace("{{CUSTOMER}}", cust)
                            .replace("{CUSTOMER}", cust);
                    }
                }
                template
                    .replace("{{FEATURE}}", &name)
                    .replace("{{FIX}}", &name)
                    .replace("{{NAME}}", &name)
                    .replace("{NAME}", &name)
            }
        }
        None => git.current_branch()?,
    };

    let mut sorted_bts = config.branch_types.clone();
    sorted_bts.sort_by_key(|b| std::cmp::Reverse(b.create.len()));

    for bt in &sorted_bts {
        let mut pattern = bt.create.clone();
        pattern = pattern.replace("{{NAME}}", ".*");
        pattern = pattern.replace("{NAME}", ".*");
        pattern = pattern.replace("{{CUSTOMER}}", ".*");
        pattern = pattern.replace("{CUSTOMER}", ".*");
        pattern = pattern.replace("{{FEATURE}}", ".*");
        pattern = pattern.replace("{{FIX}}", ".*");

        let re = Regex::new(&format!("^{}$", pattern))?;
        if re.is_match(&full_branch_name) {
            let mut resolved_bt = bt.clone();
            if let Some(ref cust) = customer_opt {
                let customer_branch = format!("customer/{}", cust);
                resolved_bt.from = customer_branch.clone();
                for to_branch in &mut resolved_bt.to {
                    to_branch.name = customer_branch.clone();
                }
            }
            return Ok((full_branch_name, resolved_bt));
        }
    }

    let target_type_name = match &customer_opt {
        Some(_) => {
            if type_name == "feature" || type_name == "hotfix" {
                format!("customer-{}", type_name)
            } else {
                type_name.to_string()
            }
        }
        None => type_name.to_string(),
    };
    let bt = config
        .branch_types
        .iter()
        .find(|b| b.name == target_type_name)
        .ok_or_else(|| {
            anyhow::anyhow!("No matching branch type for branch '{}'", full_branch_name)
        })?;

    let mut resolved_bt = bt.clone();
    if let Some(ref cust) = customer_opt {
        let customer_branch = format!("customer/{}", cust);
        resolved_bt.from = customer_branch.clone();
        for to_branch in &mut resolved_bt.to {
            to_branch.name = customer_branch.clone();
        }
    }

    Ok((full_branch_name, resolved_bt))
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    crate::utils::set_dry_run(args.dry_run);

    match &args.command {
        // -- commands that don't require env_valid --
        Command::Complete { shell } => {
            let mut cmd = Args::command();
            clap_complete::generate(*shell, &mut cmd, "gitflow", &mut std::io::stdout());
        }
        Command::List => command::list::list_branch_types(args.config),
        Command::Overview => command::overview::show_overview(args.config),
        Command::Check { file_path } => command::check::check_config(file_path.clone()),
        Command::Init => command::init::init_config(),
        Command::Continue => command::continue_cmd::continue_operation(),
        Command::Abort => command::abort_cmd::abort_operation(),

        // -- Feature branches --
        Command::Feature { action } => {
            if !env_valid() {
                return;
            }
            match action {
                FeatureAction::Start {
                    name,
                    base,
                    fetch,
                    customer,
                } => {
                    match resolve_start_branch(
                        "feature",
                        name,
                        customer.as_deref(),
                        None,
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            // If base commit/tag is specified, we can override the start source
                            let mut bt = branch_type;
                            if let Some(ref b) = base {
                                bt.from = b.clone();
                            }
                            command::start::start_task(branch_name, bt, *fetch, None);
                        }
                    }
                }
                FeatureAction::Finish {
                    name,
                    customer,
                    keep,
                    tag,
                    squash,
                    push,
                    fetch,
                    bump,
                    sign,
                    r#continue,
                    rebase: _,
                    squash_message,
                    merge_message,
                    no_verify,
                } => {
                    if *r#continue {
                        command::continue_cmd::continue_operation();
                        return;
                    }
                    match resolve_branch_info(
                        "feature",
                        name.clone(),
                        customer.clone(),
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            let opts = command::finish::FinishOptions {
                                keep: *keep,
                                tag: tag.clone(),
                                squash: *squash,
                                push: *push,
                                fetch: *fetch,
                                bump: bump.clone(),
                                squash_message: squash_message.clone(),
                                merge_message: merge_message.clone(),
                                update_message: None,
                                no_verify: *no_verify,
                                sign: *sign,
                                customer: customer.clone(),
                            };
                            command::finish::finish_task(branch_name, branch_type, opts);
                        }
                    }
                }
                FeatureAction::Update { name, rebase } => {
                    match resolve_branch_info("feature", name.clone(), None, args.config.clone()) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::update::update_branch(branch_name, branch_type, *rebase);
                        }
                    }
                }
                FeatureAction::Delete {
                    name,
                    remote,
                    force,
                } => {
                    match resolve_branch_info("feature", name.clone(), None, args.config.clone()) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::delete::delete_branch(
                                branch_name,
                                branch_type,
                                *remote,
                                *force,
                            );
                        }
                    }
                }
                FeatureAction::Rename { old_name, new_name } => {
                    match resolve_branch_info(
                        "feature",
                        Some(old_name.clone()),
                        None,
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            let new_resolved = match new_name {
                                Some(new) => new.clone(),
                                None => {
                                    // If new_name is None, old_name is treated as the new_name
                                    // and we rename the current branch
                                    let git = match Git::open() {
                                        Ok(g) => g,
                                        Err(err) => {
                                            Echo::error(err.to_string());
                                            return;
                                        }
                                    };
                                    let current = match git.current_branch() {
                                        Ok(c) => c,
                                        Err(err) => {
                                            Echo::error(err.to_string());
                                            return;
                                        }
                                    };
                                    // Make sure current branch is feature
                                    if !current.starts_with("feature/") {
                                        Echo::error("Current branch is not a feature branch");
                                        return;
                                    }
                                    let old_full = current;
                                    // If we are renaming current branch, the user invoked it as:
                                    // gitflow feature rename new-name
                                    // In this case, old_name contains the new name.
                                    let short_current =
                                        old_full.strip_prefix("feature/").unwrap_or(&old_full);
                                    let new_full = old_full.replace(short_current, old_name);
                                    command::rename::rename_branch(old_full, new_full, branch_type);
                                    return;
                                }
                            };
                            let (new_full_name, _) = match resolve_branch_info(
                                "feature",
                                Some(new_resolved),
                                None,
                                args.config.clone(),
                            ) {
                                Ok(res) => res,
                                Err(err) => {
                                    Echo::error(err.to_string());
                                    return;
                                }
                            };
                            command::rename::rename_branch(branch_name, new_full_name, branch_type);
                        }
                    }
                }
                FeatureAction::Checkout { name } => {
                    command::checkout::checkout_branch("feature", name, args.config.clone());
                }
                FeatureAction::Track { name } => {
                    match resolve_branch_info(
                        "feature",
                        Some(name.clone()),
                        None,
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::track::track_task(branch_name, branch_type);
                        }
                    }
                }
                FeatureAction::List { pattern } => {
                    command::list::list_branches("feature", pattern.clone(), args.config.clone());
                }
                FeatureAction::Publish { name } => {
                    match resolve_branch_info("feature", name.clone(), None, args.config.clone()) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::publish::publish_branch(branch_name, branch_type);
                        }
                    }
                }
            }
        }

        // -- Release branches --
        Command::Release { action } => {
            if !env_valid() {
                return;
            }
            match action {
                ReleaseAction::Start {
                    name,
                    base,
                    fetch,
                    customer,
                } => {
                    match resolve_start_branch(
                        "release",
                        name,
                        customer.as_deref(),
                        None,
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            let mut bt = branch_type;
                            if let Some(ref b) = base {
                                bt.from = b.clone();
                            }
                            command::start::start_task(branch_name, bt, *fetch, None);
                        }
                    }
                }
                ReleaseAction::Finish {
                    name,
                    customer,
                    keep,
                    tag,
                    squash,
                    push,
                    fetch,
                    bump,
                    sign,
                    r#continue,
                    rebase: _,
                    squash_message,
                    merge_message,
                    update_message,
                    no_verify,
                } => {
                    if *r#continue {
                        command::continue_cmd::continue_operation();
                        return;
                    }
                    match resolve_branch_info(
                        "release",
                        name.clone(),
                        customer.clone(),
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            let opts = command::finish::FinishOptions {
                                keep: *keep,
                                tag: tag.clone(),
                                squash: *squash,
                                push: *push,
                                fetch: *fetch,
                                bump: bump.clone(),
                                squash_message: squash_message.clone(),
                                merge_message: merge_message.clone(),
                                update_message: update_message.clone(),
                                no_verify: *no_verify,
                                sign: *sign,
                                customer: customer.clone(),
                            };
                            command::finish::finish_task(branch_name, branch_type, opts);
                        }
                    }
                }
                ReleaseAction::Update { name, rebase } => {
                    match resolve_branch_info("release", name.clone(), None, args.config.clone()) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::update::update_branch(branch_name, branch_type, *rebase);
                        }
                    }
                }
                ReleaseAction::Delete {
                    name,
                    remote,
                    force,
                } => {
                    match resolve_branch_info("release", name.clone(), None, args.config.clone()) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::delete::delete_branch(
                                branch_name,
                                branch_type,
                                *remote,
                                *force,
                            );
                        }
                    }
                }
                ReleaseAction::Rename { old_name, new_name } => {
                    match resolve_branch_info(
                        "release",
                        Some(old_name.clone()),
                        None,
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            let new_resolved = match new_name {
                                Some(new) => new.clone(),
                                None => {
                                    let git = match Git::open() {
                                        Ok(g) => g,
                                        Err(err) => {
                                            Echo::error(err.to_string());
                                            return;
                                        }
                                    };
                                    let current = match git.current_branch() {
                                        Ok(c) => c,
                                        Err(err) => {
                                            Echo::error(err.to_string());
                                            return;
                                        }
                                    };
                                    if !current.starts_with("release/") {
                                        Echo::error("Current branch is not a release branch");
                                        return;
                                    }
                                    let short_current =
                                        current.strip_prefix("release/").unwrap_or(&current);
                                    let new_full = current.replace(short_current, old_name);
                                    command::rename::rename_branch(current, new_full, branch_type);
                                    return;
                                }
                            };
                            let (new_full_name, _) = match resolve_branch_info(
                                "release",
                                Some(new_resolved),
                                None,
                                args.config.clone(),
                            ) {
                                Ok(res) => res,
                                Err(err) => {
                                    Echo::error(err.to_string());
                                    return;
                                }
                            };
                            command::rename::rename_branch(branch_name, new_full_name, branch_type);
                        }
                    }
                }
                ReleaseAction::Checkout { name } => {
                    command::checkout::checkout_branch("release", name, args.config.clone());
                }
                ReleaseAction::Track { name } => {
                    match resolve_branch_info(
                        "release",
                        Some(name.clone()),
                        None,
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::track::track_task(branch_name, branch_type);
                        }
                    }
                }
                ReleaseAction::List { pattern } => {
                    command::list::list_branches("release", pattern.clone(), args.config.clone());
                }
                ReleaseAction::Publish { name } => {
                    match resolve_branch_info("release", name.clone(), None, args.config.clone()) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::publish::publish_branch(branch_name, branch_type);
                        }
                    }
                }
            }
        }

        // -- Hotfix branches --
        Command::Hotfix { action } => {
            if !env_valid() {
                return;
            }
            match action {
                HotfixAction::Start {
                    name,
                    base,
                    fetch,
                    customer,
                } => {
                    match resolve_start_branch(
                        "hotfix",
                        name,
                        customer.as_deref(),
                        None,
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            let mut bt = branch_type;
                            if let Some(ref b) = base {
                                bt.from = b.clone();
                            }
                            command::start::start_task(branch_name, bt, *fetch, None);
                        }
                    }
                }
                HotfixAction::Finish {
                    name,
                    customer,
                    keep,
                    tag,
                    squash,
                    push,
                    fetch,
                    bump,
                    sign,
                    r#continue,
                    rebase: _,
                    squash_message,
                    merge_message,
                    no_verify,
                } => {
                    if *r#continue {
                        command::continue_cmd::continue_operation();
                        return;
                    }
                    match resolve_branch_info(
                        "hotfix",
                        name.clone(),
                        customer.clone(),
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            let opts = command::finish::FinishOptions {
                                keep: *keep,
                                tag: tag.clone(),
                                squash: *squash,
                                push: *push,
                                fetch: *fetch,
                                bump: bump.clone(),
                                squash_message: squash_message.clone(),
                                merge_message: merge_message.clone(),
                                update_message: None,
                                no_verify: *no_verify,
                                sign: *sign,
                                customer: customer.clone(),
                            };
                            command::finish::finish_task(branch_name, branch_type, opts);
                        }
                    }
                }
                HotfixAction::Update { name, rebase } => {
                    match resolve_branch_info("hotfix", name.clone(), None, args.config.clone()) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::update::update_branch(branch_name, branch_type, *rebase);
                        }
                    }
                }
                HotfixAction::Delete {
                    name,
                    remote,
                    force,
                } => match resolve_branch_info("hotfix", name.clone(), None, args.config.clone()) {
                    Err(err) => Echo::error(err.to_string()),
                    Ok((branch_name, branch_type)) => {
                        command::delete::delete_branch(branch_name, branch_type, *remote, *force);
                    }
                },
                HotfixAction::Rename { old_name, new_name } => {
                    match resolve_branch_info(
                        "hotfix",
                        Some(old_name.clone()),
                        None,
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            let new_resolved = match new_name {
                                Some(new) => new.clone(),
                                None => {
                                    let git = match Git::open() {
                                        Ok(g) => g,
                                        Err(err) => {
                                            Echo::error(err.to_string());
                                            return;
                                        }
                                    };
                                    let current = match git.current_branch() {
                                        Ok(c) => c,
                                        Err(err) => {
                                            Echo::error(err.to_string());
                                            return;
                                        }
                                    };
                                    if !current.starts_with("hotfix/") {
                                        Echo::error("Current branch is not a hotfix branch");
                                        return;
                                    }
                                    let short_current =
                                        current.strip_prefix("hotfix/").unwrap_or(&current);
                                    let new_full = current.replace(short_current, old_name);
                                    command::rename::rename_branch(current, new_full, branch_type);
                                    return;
                                }
                            };
                            let (new_full_name, _) = match resolve_branch_info(
                                "hotfix",
                                Some(new_resolved),
                                None,
                                args.config.clone(),
                            ) {
                                Ok(res) => res,
                                Err(err) => {
                                    Echo::error(err.to_string());
                                    return;
                                }
                            };
                            command::rename::rename_branch(branch_name, new_full_name, branch_type);
                        }
                    }
                }
                HotfixAction::Checkout { name } => {
                    command::checkout::checkout_branch("hotfix", name, args.config.clone());
                }
                HotfixAction::Track { name } => {
                    match resolve_branch_info(
                        "hotfix",
                        Some(name.clone()),
                        None,
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::track::track_task(branch_name, branch_type);
                        }
                    }
                }
                HotfixAction::List { pattern } => {
                    command::list::list_branches("hotfix", pattern.clone(), args.config.clone());
                }
                HotfixAction::Publish { name } => {
                    match resolve_branch_info("hotfix", name.clone(), None, args.config.clone()) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::publish::publish_branch(branch_name, branch_type);
                        }
                    }
                }
            }
        }

        // -- Custom (long-lived customer branches) --
        Command::Custom { action } => {
            if !env_valid() {
                return;
            }
            let (main_branch, customer_prefix, remote) =
                read_yqm_branch_config(args.config.clone());
            let remote_ref = remote.as_deref();

            match action {
                CustomAction::Start { name, push, fetch } => {
                    // Start creates the customer branch from main
                    if *fetch {
                        if let Ok(git) = Git::open() {
                            let r = remote.as_deref().unwrap_or("origin");
                            let _ = git.fetch_remote(r);
                        }
                    }
                    command::customer::create_customer(
                        name,
                        &main_branch,
                        if *push { remote_ref } else { None },
                    );
                }
                CustomAction::Finish { name, force } => {
                    if !force {
                        Echo::error("Customer branches are long-lived. To finish (delete) a customer branch, you must pass the --force flag.");
                        return;
                    }
                    // Delete the local customer branch
                    let git = match Git::open() {
                        Ok(g) => g,
                        Err(err) => {
                            Echo::error(err.to_string());
                            return;
                        }
                    };
                    let customer_branch = format!("customer/{}", name);
                    let finish =
                        Echo::progress(format!("delete customer branch {}", customer_branch));
                    match git.del_local_branch(&customer_branch) {
                        Err(err) => finish(false, &err.to_string()),
                        Ok(_) => finish(
                            true,
                            &format!("deleted customer branch {}", customer_branch),
                        ),
                    }
                }
                CustomAction::Sync {
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
                CustomAction::List => {
                    command::customer::list_customers(&main_branch, &customer_prefix);
                }
                CustomAction::Checkout { name } => {
                    let git = match Git::open() {
                        Ok(g) => g,
                        Err(err) => {
                            Echo::error(err.to_string());
                            return;
                        }
                    };
                    let branch = format!("customer/{}", name);
                    let finish = Echo::progress(format!("switch to customer branch {}", branch));
                    match git.switch(&branch) {
                        Err(err) => finish(false, &err.to_string()),
                        Ok(_) => finish(true, &format!("switch to customer branch {}", branch)),
                    }
                }
                CustomAction::Delete {
                    name,
                    remote: remote_del,
                    force,
                } => {
                    let bt = BranchType {
                        name: "customer".to_string(),
                        create: "customer/{NAME}".to_string(),
                        from: main_branch.clone(),
                        to: vec![],
                        remote: remote.clone(),
                        tag_pattern: None,
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
                    command::delete::delete_branch(
                        format!("customer/{}", name),
                        bt,
                        *remote_del,
                        *force,
                    );
                }
                CustomAction::Rename { old_name, new_name } => {
                    let bt = BranchType {
                        name: "customer".to_string(),
                        create: "customer/{NAME}".to_string(),
                        from: main_branch.clone(),
                        to: vec![],
                        remote: remote.clone(),
                        tag_pattern: None,
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
                    let resolved_new = match new_name {
                        Some(new) => format!("customer/{}", new),
                        None => {
                            // Rename current branch (old_name is the new name)
                            let git = match Git::open() {
                                Ok(g) => g,
                                Err(err) => {
                                    Echo::error(err.to_string());
                                    return;
                                }
                            };
                            let current = match git.current_branch() {
                                Ok(c) => c,
                                Err(err) => {
                                    Echo::error(err.to_string());
                                    return;
                                }
                            };
                            if !current.starts_with("customer/") {
                                Echo::error("Current branch is not a customer branch");
                                return;
                            }
                            command::rename::rename_branch(
                                current,
                                format!("customer/{}", old_name),
                                bt,
                            );
                            return;
                        }
                    };
                    command::rename::rename_branch(
                        format!("customer/{}", old_name),
                        resolved_new,
                        bt,
                    );
                }
            }
        }

        // -- General branches --
        Command::General { action } => {
            if !env_valid() {
                return;
            }
            match action {
                GeneralAction::Start {
                    name,
                    customer,
                    to,
                    pick,
                    fetch,
                } => {
                    match resolve_start_branch(
                        "general",
                        name,
                        Some(customer),
                        Some(to),
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            // Split comma/space-separated pick commits
                            let commits: Vec<String> = pick
                                .split([',', ' ', '\t', '\n'])
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();

                            if commits.is_empty() {
                                Echo::error("Error: --pick commits list cannot be empty");
                                return;
                            }

                            command::start::start_task(
                                branch_name,
                                branch_type,
                                *fetch,
                                Some(commits),
                            );
                        }
                    }
                }
                GeneralAction::Finish {
                    name,
                    keep,
                    no_verify,
                    r#continue,
                } => {
                    if *r#continue {
                        command::continue_cmd::continue_operation();
                        return;
                    }
                    match resolve_branch_info("general", name.clone(), None, args.config.clone()) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            let opts = command::finish::FinishOptions {
                                keep: *keep,
                                tag: None,
                                squash: true, // General branches always squash merge to main/customer
                                push: true,
                                fetch: false,
                                bump: None,
                                squash_message: None,
                                merge_message: None,
                                update_message: None,
                                no_verify: *no_verify,
                                sign: false,
                                customer: None,
                            };
                            command::finish::finish_task(branch_name, branch_type, opts);
                        }
                    }
                }
                GeneralAction::Update { name, rebase } => {
                    match resolve_branch_info("general", name.clone(), None, args.config.clone()) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::update::update_branch(branch_name, branch_type, *rebase);
                        }
                    }
                }
                GeneralAction::Delete {
                    name,
                    remote,
                    force,
                } => {
                    match resolve_branch_info("general", name.clone(), None, args.config.clone()) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::delete::delete_branch(
                                branch_name,
                                branch_type,
                                *remote,
                                *force,
                            );
                        }
                    }
                }
                GeneralAction::Rename { old_name, new_name } => {
                    match resolve_branch_info(
                        "general",
                        Some(old_name.clone()),
                        None,
                        args.config.clone(),
                    ) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            let new_resolved = match new_name {
                                Some(new) => new.clone(),
                                None => {
                                    let git = match Git::open() {
                                        Ok(g) => g,
                                        Err(err) => {
                                            Echo::error(err.to_string());
                                            return;
                                        }
                                    };
                                    let current = match git.current_branch() {
                                        Ok(c) => c,
                                        Err(err) => {
                                            Echo::error(err.to_string());
                                            return;
                                        }
                                    };
                                    if !current.starts_with("general/")
                                        && !current.starts_with("generalize/")
                                    {
                                        Echo::error("Current branch is not a general branch");
                                        return;
                                    }
                                    let prefix = if current.starts_with("general/") {
                                        "general/"
                                    } else {
                                        "generalize/"
                                    };
                                    let short_current =
                                        current.strip_prefix(prefix).unwrap_or(&current);
                                    let new_full = current.replace(short_current, old_name);
                                    command::rename::rename_branch(current, new_full, branch_type);
                                    return;
                                }
                            };
                            let (new_full_name, _) = match resolve_branch_info(
                                "general",
                                Some(new_resolved),
                                None,
                                args.config.clone(),
                            ) {
                                Ok(res) => res,
                                Err(err) => {
                                    Echo::error(err.to_string());
                                    return;
                                }
                            };
                            command::rename::rename_branch(branch_name, new_full_name, branch_type);
                        }
                    }
                }
                GeneralAction::Checkout { name } => {
                    command::checkout::checkout_branch("general", name, args.config.clone());
                }
                GeneralAction::List { pattern } => {
                    command::list::list_branches("general", pattern.clone(), args.config.clone());
                }
                GeneralAction::Publish { name } => {
                    match resolve_branch_info("general", name.clone(), None, args.config.clone()) {
                        Err(err) => Echo::error(err.to_string()),
                        Ok((branch_name, branch_type)) => {
                            command::publish::publish_branch(branch_name, branch_type);
                        }
                    }
                }
            }
        }

        // -- Global Finish shorthand --
        Command::Finish {
            keep,
            tag,
            squash,
            push,
            fetch,
            customer,
            bump,
            sign,
            r#continue,
            rebase: _,
            squash_message,
            merge_message,
            update_message,
            no_verify,
        } => {
            if !env_valid() {
                return;
            }
            if *r#continue {
                command::continue_cmd::continue_operation();
                return;
            }
            let git = match Git::open() {
                Ok(g) => g,
                Err(err) => {
                    Echo::error(err.to_string());
                    return;
                }
            };
            let current = match git.current_branch() {
                Ok(c) => c,
                Err(err) => {
                    Echo::error(err.to_string());
                    return;
                }
            };
            match get_branch_type_name(current.clone(), None, args.config.clone()) {
                Err(err) => Echo::error(err.to_string()),
                Ok((branch_name, mut branch_type)) => {
                    if let Some(customer_name) = customer {
                        let customer_branch = format!("customer/{}", customer_name);
                        branch_type.from = customer_branch.clone();
                        for to in &mut branch_type.to {
                            to.name = customer_branch.clone();
                        }
                    }
                    let opts = command::finish::FinishOptions {
                        keep: *keep,
                        tag: tag.clone(),
                        squash: *squash,
                        push: *push,
                        fetch: *fetch,
                        bump: bump.clone(),
                        squash_message: squash_message.clone(),
                        merge_message: merge_message.clone(),
                        update_message: update_message.clone(),
                        no_verify: *no_verify,
                        sign: *sign,
                        customer: customer.clone(),
                    };
                    command::finish::finish_task(branch_name, branch_type, opts);
                }
            }
        }

        // -- Global Update shorthand --
        Command::Update { rebase } => {
            if !env_valid() {
                return;
            }
            let git = match Git::open() {
                Ok(g) => g,
                Err(err) => {
                    Echo::error(err.to_string());
                    return;
                }
            };
            let current = match git.current_branch() {
                Ok(c) => c,
                Err(err) => {
                    Echo::error(err.to_string());
                    return;
                }
            };
            match get_branch_type_name(current.clone(), None, args.config.clone()) {
                Err(err) => Echo::error(err.to_string()),
                Ok((branch_name, branch_type)) => {
                    command::update::update_branch(branch_name, branch_type, *rebase);
                }
            }
        }

        // -- Global Delete shorthand --
        Command::Delete { remote, force } => {
            if !env_valid() {
                return;
            }
            let git = match Git::open() {
                Ok(g) => g,
                Err(err) => {
                    Echo::error(err.to_string());
                    return;
                }
            };
            let current = match git.current_branch() {
                Ok(c) => c,
                Err(err) => {
                    Echo::error(err.to_string());
                    return;
                }
            };
            match get_branch_type_name(current.clone(), None, args.config.clone()) {
                Err(err) => Echo::error(err.to_string()),
                Ok((branch_name, branch_type)) => {
                    command::delete::delete_branch(branch_name, branch_type, *remote, *force);
                }
            }
        }

        // -- Global Rename shorthand --
        Command::Rename { new_name } => {
            if !env_valid() {
                return;
            }
            let git = match Git::open() {
                Ok(g) => g,
                Err(err) => {
                    Echo::error(err.to_string());
                    return;
                }
            };
            let current = match git.current_branch() {
                Ok(c) => c,
                Err(err) => {
                    Echo::error(err.to_string());
                    return;
                }
            };
            match get_branch_type_name(current.clone(), None, args.config.clone()) {
                Err(err) => Echo::error(err.to_string()),
                Ok((branch_name, branch_type)) => {
                    let mut prefix = "".to_string();
                    if branch_name.starts_with("feature/") {
                        prefix = "feature/".to_string();
                    } else if branch_name.starts_with("hotfix/") {
                        prefix = "hotfix/".to_string();
                    } else if branch_name.starts_with("release/") {
                        prefix = "release/".to_string();
                    } else if branch_name.starts_with("general/") {
                        prefix = "general/".to_string();
                    } else if branch_name.starts_with("generalize/") {
                        prefix = "generalize/".to_string();
                    } else if branch_name.starts_with("customer/") {
                        prefix = "customer/".to_string();
                    }

                    let new_full_name = format!("{}{}", prefix, new_name);
                    command::rename::rename_branch(branch_name, new_full_name, branch_type);
                }
            }
        }

        // -- Global Publish shorthand --
        Command::Publish { name } => {
            if !env_valid() {
                return;
            }
            let git = match Git::open() {
                Ok(g) => g,
                Err(err) => {
                    Echo::error(err.to_string());
                    return;
                }
            };
            let branch_name = match name {
                Some(ref n) => {
                    if n.contains('/') {
                        n.clone()
                    } else {
                        match git.get_local_branches() {
                            Ok(branches) => {
                                let matches: Vec<String> = branches
                                    .into_iter()
                                    .filter(|b| b == n || b.ends_with(&format!("/{}", n)))
                                    .collect();
                                if matches.len() == 1 {
                                    matches[0].clone()
                                } else if matches.is_empty() {
                                    Echo::error(format!(
                                        "Could not find local branch matching '{}'",
                                        n
                                    ));
                                    return;
                                } else {
                                    Echo::error(format!(
                                        "Ambiguous branch name '{}'. Matches: {:?}",
                                        n, matches
                                    ));
                                    return;
                                }
                            }
                            Err(err) => {
                                Echo::error(err.to_string());
                                return;
                            }
                        }
                    }
                }
                None => match git.current_branch() {
                    Ok(c) => c,
                    Err(err) => {
                        Echo::error(err.to_string());
                        return;
                    }
                },
            };

            match get_branch_type_name(branch_name.clone(), None, args.config.clone()) {
                Err(err) => Echo::error(err.to_string()),
                Ok((resolved_branch, branch_type)) => {
                    command::publish::publish_branch(resolved_branch, branch_type);
                }
            }
        }

        // -- Deprecated legacy commands (just in case they are called) --
        Command::Sync { target, strategy } => {
            if !env_valid() {
                return;
            }
            command::sync::sync_repo_branches(
                target.clone(),
                strategy.clone().unwrap_or(cli::SyncStrategy::Increment),
            );
        }
    }
}

fn read_yqm_branch_config(
    config_path: Option<std::path::PathBuf>,
) -> (String, String, Option<String>) {
    let default_main = "main".to_string();
    let default_prefix = "customer/".to_string();

    let config_text = if let Some(path) = config_path {
        std::fs::read_to_string(path).ok()
    } else {
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
            let remote = yqm.hooks.post_start.as_ref().map(|_| "origin".to_string());
            (yqm.branches.main, yqm.branches.customer.prefix, remote)
        }
        Err(_) => (default_main, default_prefix, None),
    }
}
