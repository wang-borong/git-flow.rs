use regex::Regex;
use std::path::PathBuf;

use crate::{echo::Echo, git::Git};

pub fn checkout_branch(type_name: &str, name: &str, config_path: Option<PathBuf>) {
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

    let config = match crate::config::read::read_config(config_path) {
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

    let mut matches = Vec::new();

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
            if re.is_match(branch) {
                // If it contains the query 'name' (case-insensitive)
                if branch.to_lowercase().contains(&name.to_lowercase()) {
                    matches.push(branch.clone());
                }
                break;
            }
        }
    }

    if matches.is_empty() {
        Echo::error(format!(
            "No branches matching '{}' found for type '{}'",
            name, type_name
        ));
    } else if matches.len() == 1 {
        let branch_to_switch = &matches[0];
        let finish = Echo::progress(format!("switch to branch {}", branch_to_switch));
        match git.switch(branch_to_switch) {
            Err(err) => {
                finish(false, &err.to_string());
            }
            Ok(_) => {
                finish(true, &format!("switch to branch {}", branch_to_switch));
            }
        }
    } else {
        Echo::warning(format!("Multiple branches match '{}':", name));
        for m in &matches {
            println!("  {}", m);
        }
        Echo::info("Please be more specific.");
    }
}
