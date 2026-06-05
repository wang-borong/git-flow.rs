use std::{
    io::{self, Write},
    path::PathBuf,
    process,
};

use anyhow::{bail, Result};
use regex::Regex;

use crate::{
    config::{
        definition::{BranchType, Command, BRANCH_NAME_PLACEHOLDER},
        read::read_config,
    },
    echo::Echo,
    git::Git,
};

use std::sync::atomic::{AtomicBool, Ordering};

pub static DRY_RUN: AtomicBool = AtomicBool::new(false);

pub fn set_dry_run(val: bool) {
    DRY_RUN.store(val, Ordering::SeqCst);
}

pub fn is_dry_run() -> bool {
    DRY_RUN.load(Ordering::SeqCst)
}

pub fn env_valid() -> bool {
    if !Git::git_installed() {
        Echo::error("not in a git project (or git2 unavailable)");
        return false;
    }

    true
}

pub fn get_branch_type_name(
    branch_name: String,
    branch_type: Option<String>,
    config_path: Option<PathBuf>,
) -> Result<(
    /* branch_name */ String,
    /* branch_type */ BranchType,
)> {
    let config = read_config(config_path)?;

    if let Some(branch_type_v) = branch_type {
        let target_branch_type = config.branch_types.iter().find(|x| x.name == branch_type_v);
        match target_branch_type {
            None => bail!("no matched branch type"),
            Some(target_branch_type_v) => {
                return Ok((
                    target_branch_type_v
                        .create
                        .replace(BRANCH_NAME_PLACEHOLDER, &branch_name),
                    target_branch_type_v.clone(),
                ))
            }
        }
    }

    let target_branch_type = config.branch_types.iter().find(|x| {
        let mut pattern = x.create.clone();
        pattern = pattern.replace("{{NAME}}", ".*");
        pattern = pattern.replace("{NAME}", ".*");
        pattern = pattern.replace("{{CUSTOMER}}", ".*");
        pattern = pattern.replace("{CUSTOMER}", ".*");
        pattern = pattern.replace("{{FEATURE}}", ".*");
        pattern = pattern.replace("{FEATURE}", ".*");
        pattern = pattern.replace("{{FIX}}", ".*");
        pattern = pattern.replace("{FIX}", ".*");
        pattern = pattern.replace("{{RELEASE}}", ".*");
        pattern = pattern.replace("{RELEASE}", ".*");

        // ISSUE-UT1: Handle invalid regex in config gracefully
        match Regex::new(&format!("^{}$", pattern)) {
            Ok(regex) => regex.is_match(&branch_name),
            Err(e) => {
                Echo::error(format!(
                    "Invalid regex pattern '{}' in config: {}",
                    pattern, e
                ));
                false
            }
        }
    });
    match target_branch_type {
        None => bail!("no matched branch type"),
        Some(target_branch_type_v) => {
            let mut resolved_bt = target_branch_type_v.clone();

            // Try to resolve customer using git config or fallback
            let mut customer_val = None;
            if let Ok(git) = Git::open() {
                if let Ok(Some(cust)) = git.get_branch_config(&branch_name, "gitflow-customer") {
                    customer_val = Some(cust);
                } else {
                    // Fallback detection from existing customer branches
                    let customer_prefix = config
                        .branch_types
                        .iter()
                        .find(|b| b.name == "customer-feature")
                        .map(|b| b.from.replace("{{NAME}}", ""))
                        .unwrap_or_else(|| "customer/".to_string());
                    if let Ok(branches) = git.get_local_branches() {
                        for b in branches {
                            if b.starts_with(&customer_prefix) {
                                if let Some(cust) = b.strip_prefix(&customer_prefix) {
                                    if !cust.is_empty() && branch_name.contains(cust) {
                                        customer_val = Some(cust.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            resolved_bt.resolve_customer(&branch_name, customer_val.as_deref());
            Ok((branch_name, resolved_bt))
        }
    }
}

pub fn run_hook(
    command: Option<Command>,
    branch_name: &str,
    branch_type: &BranchType,
) -> Result<()> {
    let command = match command {
        Some(command_v) => command_v,
        None => return Ok(()),
    };

    // -- map args --
    let regex = Regex::new(&branch_type.create.replace(BRANCH_NAME_PLACEHOLDER, "(.*)")).unwrap();
    let short_branch_name = match regex.captures(branch_name) {
        None => None,
        Some(captures) => captures.get(1),
    };
    let args = match short_branch_name {
        None => command.args,
        Some(name) => command
            .args
            .iter()
            .map(|x| {
                x.replace(BRANCH_NAME_PLACEHOLDER, name.as_str())
                    .to_string()
            })
            .collect::<Vec<String>>(),
    };

    let msg = format!("Run hook: {} {}", command.command, args.join(" "));
    if is_dry_run() {
        Echo::info(format!(
            "[Dry Run] Would run hook: {} {}",
            command.command,
            args.join(" ")
        ));
        return Ok(());
    }

    // -- prompt user for confirmation if stdin is a terminal --
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        print!(
            "Do you want to run the hook command '{} {}'? [y/N]: ",
            command.command,
            args.join(" ")
        );
        let _ = io::stdout().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let trimmed = input.trim().to_lowercase();
            if trimmed != "y" && trimmed != "yes" {
                bail!("Hook execution declined by user");
            }
        } else {
            bail!("Failed to read user input");
        }
    }

    let finish = Echo::progress(&msg);

    // -- run --

    let result = process::Command::new(command.command).args(args).output();

    // -- print result --
    let output = match result {
        Err(err) => {
            finish(false, &err.to_string());
            bail!("");
        }
        Ok(output_v) => output_v,
    };
    match output.status.success() {
        false => {
            finish(false, &String::from_utf8(output.stderr).unwrap());
            bail!("");
        }
        true => {
            finish(true, &msg);
            Ok(())
        }
    }
}

pub fn bump_version(current_version: &str, bump_type: &str) -> Result<String> {
    let re = Regex::new(r"^(v?)(\d+)\.(\d+)\.(\d+)(?:-([a-zA-Z0-9.-]+))?$")?;
    let caps = match re.captures(current_version) {
        Some(c) => c,
        None => bail!(
            "version '{}' does not match semver pattern",
            current_version
        ),
    };

    let prefix = caps.get(1).map_or("", |m| m.as_str());
    let major: u32 = caps.get(2).unwrap().as_str().parse()?;
    let mut minor: u32 = caps.get(3).unwrap().as_str().parse()?;
    let mut patch: u32 = caps.get(4).unwrap().as_str().parse()?;
    let suffix = caps.get(5).map(|m| m.as_str());

    match bump_type {
        "minor" => {
            minor += 1;
            patch = 0;
            if let Some(s) = suffix {
                let suffix_re = Regex::new(r"^(.*?)\.(\d+)$")?;
                if let Some(suffix_caps) = suffix_re.captures(s) {
                    let base_suffix = suffix_caps.get(1).unwrap().as_str();
                    return Ok(format!(
                        "{}{}.{}.{}-{}.1",
                        prefix, major, minor, patch, base_suffix
                    ));
                }
                return Ok(format!("{}{}.{}.{}-{}", prefix, major, minor, patch, s));
            }
            Ok(format!("{}{}.{}.{}", prefix, major, minor, patch))
        }
        "patch" => {
            if let Some(s) = suffix {
                let suffix_re = Regex::new(r"^(.*?)\.(\d+)$")?;
                if let Some(suffix_caps) = suffix_re.captures(s) {
                    let base_suffix = suffix_caps.get(1).unwrap().as_str();
                    let build_num: u32 = suffix_caps.get(2).unwrap().as_str().parse()?;
                    return Ok(format!(
                        "{}{}.{}.{}-{}.{}",
                        prefix,
                        major,
                        minor,
                        patch,
                        base_suffix,
                        build_num + 1
                    ));
                }
                patch += 1;
                return Ok(format!("{}{}.{}.{}-{}", prefix, major, minor, patch, s));
            }
            patch += 1;
            Ok(format!("{}{}.{}.{}", prefix, major, minor, patch))
        }
        _ => bail!("unknown bump type '{}'", bump_type),
    }
}

pub fn find_latest_tag_and_bump(
    git: &Git,
    customer: Option<&str>,
    bump_type: &str,
) -> Result<String> {
    let tags = git.get_tags()?;
    let re = Regex::new(r"^(v?)(\d+)\.(\d+)\.(\d+)(?:-([a-zA-Z0-9.-]+))?$")?;
    let mut parsed_tags = Vec::new();

    for tag in &tags {
        if let Some(caps) = re.captures(tag) {
            let prefix = caps.get(1).map_or("", |m| m.as_str());
            let major: u32 = caps.get(2).unwrap().as_str().parse().unwrap_or(0);
            let minor: u32 = caps.get(3).unwrap().as_str().parse().unwrap_or(0);
            let patch: u32 = caps.get(4).unwrap().as_str().parse().unwrap_or(0);
            let suffix = caps.get(5).map(|m| m.as_str().to_string());

            match customer {
                Some(c) => {
                    if let Some(ref s) = suffix {
                        if s.starts_with(c) {
                            parsed_tags.push((
                                major,
                                minor,
                                patch,
                                suffix.clone(),
                                prefix.to_string(),
                                tag.clone(),
                            ));
                        }
                    }
                }
                None => {
                    if suffix.is_none() {
                        parsed_tags.push((
                            major,
                            minor,
                            patch,
                            None,
                            prefix.to_string(),
                            tag.clone(),
                        ));
                    }
                }
            }
        }
    }

    if parsed_tags.is_empty() {
        let default_tag = match customer {
            Some(c) => format!("v1.0.0-{}.1", c),
            None => "v1.0.0".to_string(),
        };
        Echo::info(format!(
            "No existing tags found. Starting from default: {}",
            default_tag
        ));
        return Ok(default_tag);
    }

    parsed_tags.sort_by(|a, b| {
        if a.0 != b.0 {
            return b.0.cmp(&a.0);
        }
        if a.1 != b.1 {
            return b.1.cmp(&a.1);
        }
        if a.2 != b.2 {
            return b.2.cmp(&a.2);
        }

        match (&a.3, &b.3) {
            (Some(sa), Some(sb)) => {
                let suffix_re = Regex::new(r"^(.*?)\.(\d+)$").unwrap();
                let a_num = suffix_re
                    .captures(sa)
                    .and_then(|c| c.get(2))
                    .and_then(|m| m.as_str().parse::<u32>().ok())
                    .unwrap_or(0);
                let b_num = suffix_re
                    .captures(sb)
                    .and_then(|c| c.get(2))
                    .and_then(|m| m.as_str().parse::<u32>().ok())
                    .unwrap_or(0);
                b_num.cmp(&a_num)
            }
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    let latest_tag = &parsed_tags[0].5;
    let bumped = bump_version(latest_tag, bump_type)?;
    Echo::info(format!(
        "Latest tag found: {}. Bumped to: {}",
        latest_tag, bumped
    ));
    Ok(bumped)
}
