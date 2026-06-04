use std::path::PathBuf;

use anyhow::{Context, Result};
use tabled::{
    settings::{peaker::PriorityMax, Width},
    Table, Tabled,
};
use terminal_size::{terminal_size, Height as TerminalHeight, Width as TerminalWidth};

use crate::{
    config::{definition::Command, read::read_config},
    echo::Echo,
    git::Git,
};
use regex::Regex;

#[derive(Tabled)]
struct BranchType {
    name: String,
    create: String,
    from: String,
    to: String,
    remote: String,
    tag_pattern: String,
    before_start: String,
    after_start: String,
    before_finish: String,
    after_finish: String,
    before_drop: String,
    after_drop: String,
}

pub fn list_branch_types(config_path: Option<PathBuf>) {
    let config = read_config(config_path);

    match config {
        Err(err) => {
            Echo::error(err.to_string());
        }
        Ok(config_v) => {
            if config_v.branch_types.is_empty() {
                Echo::warning("no branch types avaliable");
                return;
            }

            let branch_types: Vec<BranchType> = config_v
                .branch_types
                .iter()
                .map(|x| BranchType {
                    name: x.name.clone(),
                    create: x.create.clone(),
                    from: x.from.clone(),
                    to: x
                        .to
                        .iter()
                        .map(|y| y.name.clone())
                        .collect::<Vec<String>>()
                        .join(", "),
                    remote: x.remote.clone().unwrap_or_default(),
                    tag_pattern: x.tag_pattern.clone().unwrap_or_default(),
                    before_start: command_to_string(x.before_start.clone()),
                    after_start: command_to_string(x.after_start.clone()),
                    before_finish: command_to_string(x.before_finish.clone()),
                    after_finish: command_to_string(x.after_finish.clone()),
                    before_drop: command_to_string(x.before_drop.clone()),
                    after_drop: command_to_string(x.after_drop.clone()),
                })
                .collect();

            let width = match get_terminal_size() {
                Err(_) => {
                    Echo::error("unable to get terminal size");
                    return;
                }
                Ok(size) => size.0,
            };
            println!(
                "{}",
                Table::new(branch_types)
                    .with(Width::wrap(width).priority(PriorityMax::default()))
                    .with(Width::increase(width))
            )
        }
    }
}

fn command_to_string(command: Option<Command>) -> String {
    match command {
        None => String::new(),
        Some(command_v) => format!("{} {}", command_v.command, command_v.args.join(" ")),
    }
}

fn get_terminal_size() -> Result<(usize, usize)> {
    let (TerminalWidth(width), TerminalHeight(height)) = terminal_size().context("unable")?;

    Ok((width as usize, height as usize))
}

pub fn list_branches(type_name: &str, pattern: Option<String>, config_path: Option<PathBuf>) {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    let branches = match git.get_local_branches() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(b) => b,
    };

    let config = match read_config(config_path) {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(c) => c,
    };

    let target_types: Vec<&crate::config::definition::BranchType> = config
        .branch_types
        .iter()
        .filter(|b| b.name == type_name || b.name == format!("customer-{}", type_name))
        .collect();

    if target_types.is_empty() {
        Echo::warning(format!(
            "No configured prefix found for branch type '{}'",
            type_name
        ));
        return;
    }

    let mut matching_branches = Vec::new();

    for branch in &branches {
        for bt in &target_types {
            let mut pattern_str = bt.create.clone();
            pattern_str = pattern_str.replace("{{NAME}}", "([^/]+)");
            pattern_str = pattern_str.replace("{NAME}", "([^/]+)");
            pattern_str = pattern_str.replace("{{CUSTOMER}}", "([^/]+)");
            pattern_str = pattern_str.replace("{CUSTOMER}", "([^/]+)");
            pattern_str = pattern_str.replace("{{FEATURE}}", "(.*)");
            pattern_str = pattern_str.replace("{{FIX}}", "(.*)");

            let re = Regex::new(&format!("^{}$", pattern_str)).unwrap();
            if let Some(caps) = re.captures(branch) {
                if let Some(short_name_cap) = caps.iter().last().flatten() {
                    let short_name = short_name_cap.as_str();

                    let is_match = match &pattern {
                        Some(p) => {
                            let wildcard_pattern =
                                format!("^{}$", p.replace("*", ".*").replace("?", "."));
                            if let Ok(re_p) = Regex::new(&wildcard_pattern) {
                                re_p.is_match(short_name)
                            } else {
                                false
                            }
                        }
                        None => true,
                    };

                    if is_match {
                        matching_branches.push(branch.clone());
                    }
                }
                break;
            }
        }
    }

    if matching_branches.is_empty() {
        Echo::info("No matching branches found");
    } else {
        println!("\nLocal {} branches:", type_name);
        for b in matching_branches {
            println!("  {}", b);
        }
        println!();
    }
}
