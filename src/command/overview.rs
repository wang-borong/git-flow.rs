use regex::Regex;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::{
    config::{definition::BranchType, read::read_config},
    echo::Echo,
    git::Git,
};

fn get_parent_branch(branch_name: &str, bt: &BranchType) -> String {
    let mut pattern = bt.create.clone();
    pattern = pattern.replace("{{CUSTOMER}}", "(?P<customer>[^/]+)");
    pattern = pattern.replace("{CUSTOMER}", "(?P<customer>[^/]+)");
    pattern = pattern.replace("{{NAME}}", "(?P<name>[^/]+)");
    pattern = pattern.replace("{NAME}", "(?P<name>[^/]+)");
    pattern = pattern.replace("{{FEATURE}}", "(?P<feature>.+)");
    pattern = pattern.replace("{FEATURE}", "(?P<feature>.+)");
    pattern = pattern.replace("{{FIX}}", "(?P<fix>.+)");
    pattern = pattern.replace("{FIX}", "(?P<fix>.+)");

    let re = match Regex::new(&format!("^{}$", pattern)) {
        Ok(r) => r,
        Err(_) => return bt.from.clone(),
    };

    if let Some(caps) = re.captures(branch_name) {
        let mut resolved_from = bt.from.clone();
        if let Some(m) = caps.name("customer") {
            resolved_from = resolved_from
                .replace("{{CUSTOMER}}", m.as_str())
                .replace("{CUSTOMER}", m.as_str());
        }
        if let Some(m) = caps.name("name") {
            resolved_from = resolved_from
                .replace("{{NAME}}", m.as_str())
                .replace("{NAME}", m.as_str());
        }
        resolved_from
    } else {
        bt.from.clone()
    }
}

pub fn show_overview(config_path: Option<PathBuf>) {
    let git = match Git::open() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(git) => git,
    };

    let config = match read_config(config_path) {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(c) => c,
    };

    let current_branch = match git.current_branch() {
        Ok(c) => c,
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
    };

    let local_branches = match git.get_local_branches() {
        Err(err) => {
            Echo::error(err.to_string());
            return;
        }
        Ok(b) => b,
    };

    let local_branches_set: HashSet<String> = local_branches.iter().cloned().collect();

    // 1. Gather all base branches
    let mut base_branches = HashSet::new();
    for bt in &config.branch_types {
        base_branches.insert(bt.from.clone());
    }

    println!("\n\x1b[1;36m┌────────────────────────────────────────────────────────┐\x1b[0m");
    println!("\x1b[1;36m│               GITFLOW WORKSPACE OVERVIEW               │\x1b[0m");
    println!("\x1b[1;36m└────────────────────────────────────────────────────────┘\x1b[0m");

    // Group branches by branch type name
    let mut grouped_branches: std::collections::HashMap<String, Vec<(String, String)>> =
        std::collections::HashMap::new();
    let mut other_branches = Vec::new();
    let mut core_branches = Vec::new();

    for branch in &local_branches {
        // Try to match against branch types
        let mut matched = false;
        for bt in &config.branch_types {
            let mut pattern_str = bt.create.clone();
            pattern_str = pattern_str.replace("{{NAME}}", "([^/]+)");
            pattern_str = pattern_str.replace("{NAME}", "([^/]+)");
            pattern_str = pattern_str.replace("{{CUSTOMER}}", "([^/]+)");
            pattern_str = pattern_str.replace("{CUSTOMER}", "([^/]+)");
            pattern_str = pattern_str.replace("{{FEATURE}}", "(.*)");
            pattern_str = pattern_str.replace("{{FIX}}", "(.*)");

            if let Ok(re) = Regex::new(&format!("^{}$", pattern_str)) {
                if re.is_match(branch) {
                    let parent = get_parent_branch(branch, bt);
                    grouped_branches
                        .entry(bt.name.clone())
                        .or_default()
                        .push((branch.clone(), parent));
                    matched = true;
                    break;
                }
            }
        }

        if !matched {
            if base_branches.contains(branch)
                || branch == "main"
                || branch == "master"
                || branch == "dev"
                || branch == "develop"
            {
                core_branches.push(branch.clone());
            } else {
                other_branches.push(branch.clone());
            }
        }
    }

    // 2. Print Core/Base Branches
    if !core_branches.is_empty() {
        println!("\n\x1b[1;34m🌿 Core / Base Branches\x1b[0m");
        println!("\x1b[90m────────────────────────────────────────────────────────\x1b[0m");
        for branch in &core_branches {
            let is_head = branch == &current_branch;

            // Core branches usually don't have parents configured directly,
            // but dev might track main. Let's try to parse if there's any relation.
            let sync_status = if branch == "dev" || branch == "develop" {
                let parent = if local_branches_set.contains("main") {
                    "main"
                } else if local_branches_set.contains("master") {
                    "master"
                } else {
                    ""
                };
                if !parent.is_empty() {
                    match git.get_ahead_behind(branch, parent) {
                        Ok((ahead, behind)) => {
                            if ahead == 0 && behind == 0 {
                                format!("Up-to-date with {}", parent)
                            } else {
                                format!("{} ahead, {} behind {}", ahead, behind, parent)
                            }
                        }
                        Err(_) => "Sync status unknown".to_string(),
                    }
                } else {
                    "Standalone".to_string()
                }
            } else {
                "Standalone".to_string()
            };

            if is_head {
                println!(
                    "  \x1b[1;32m🟢 * {:<25} \x1b[0m\x1b[90m[{}]\x1b[0m",
                    branch, sync_status
                );
            } else {
                println!("     {:<25} \x1b[90m[{}]\x1b[0m", branch, sync_status);
            }
        }
    }

    // 3. Print configured branch types and their active branches
    for bt in &config.branch_types {
        let name = &bt.name;
        println!(
            "\n\x1b[1;35m🎯 {} Branches\x1b[0m \x1b[90m(Pattern: {})\x1b[0m",
            name.to_uppercase(),
            bt.create
        );
        println!("\x1b[90m────────────────────────────────────────────────────────\x1b[0m");

        match grouped_branches.get(name) {
            None => {
                println!("  \x1b[90m(No active branches)\x1b[0m");
            }
            Some(branches) => {
                for (branch, parent) in branches {
                    let is_head = branch == &current_branch;

                    let sync_str = if local_branches_set.contains(parent) {
                        match git.get_ahead_behind(branch, parent) {
                            Ok((ahead, behind)) => {
                                if ahead == 0 && behind == 0 {
                                    format!("Up-to-date with {}", parent)
                                } else if ahead > 0 && behind > 0 {
                                    format!("{} ahead, {} behind {}", ahead, behind, parent)
                                } else if ahead > 0 {
                                    format!("{} ahead of {}", ahead, parent)
                                } else {
                                    format!("{} behind {}", behind, parent)
                                }
                            }
                            Err(_) => format!("Sync unknown with {}", parent),
                        }
                    } else {
                        format!("Base branch {} is not local", parent)
                    };

                    if is_head {
                        println!(
                            "  \x1b[1;32m🟢 * {:<25} \x1b[0m\x1b[90m→ {}\x1b[0m",
                            branch, sync_str
                        );
                    } else {
                        println!("     {:<25} \x1b[90m→ {}\x1b[0m", branch, sync_str);
                    }
                }
            }
        }
    }

    // 4. Print other/untracked branches
    if !other_branches.is_empty() {
        println!("\n\x1b[1;36m👤 Other / Untracked Branches\x1b[0m");
        println!("\x1b[90m────────────────────────────────────────────────────────\x1b[0m");
        for branch in &other_branches {
            let is_head = branch == &current_branch;
            if is_head {
                println!("  \x1b[1;32m🟢 * {}\x1b[0m", branch);
            } else {
                println!("     {}", branch);
            }
        }
    }

    println!("\n\x1b[90m────────────────────────────────────────────────────────\x1b[0m");
    if let Ok(changes) = git.has_uncommitted_changes() {
        if changes {
            println!(
                "  \x1b[1;33m⚠️  Warning: You have uncommitted changes in your workspace.\x1b[0m"
            );
        } else {
            println!("  \x1b[1;32m🟢 Workspace is clean.\x1b[0m");
        }
    }
    println!("\x1b[90m────────────────────────────────────────────────────────\x1b[0m\n");
}
