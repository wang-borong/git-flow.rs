use anyhow::{bail, Result};
use regex::Regex;

use super::definition::{Config, BRANCH_NAME_PLACEHOLDER};

#[cfg(test)]
mod test;

pub fn validate_config(config: &Config) -> Result<()> {
    no_duplicate_branch_type(config)?;
    target_is_valid_regex(config)?;
    create_is_valid(config)?;
    Ok(())
}

fn no_duplicate_branch_type(config: &Config) -> Result<()> {
    let branch_types = &config.branch_types;

    for (i, branch_type_i) in branch_types.iter().enumerate() {
        for branch_type_j in &branch_types[..i] {
            if branch_type_i.name == branch_type_j.name {
                bail!(
                    "invalid config: duplicate branch type name {}",
                    &branch_type_i.name
                );
            }

            if branch_type_i.create == branch_type_j.create {
                bail!(
                    "invalid config: duplicate branch type create {}",
                    &branch_type_i.create
                );
            }
        }
    }

    Ok(())
}

fn target_is_valid_regex(config: &Config) -> Result<()> {
    for i in 0..config.branch_types.len() {
        let branch_type = &config.branch_types[i];

        for j in 0..branch_type.to.len() {
            let target = &branch_type.to[j];

            if Regex::new(&target.name).is_err() {
                bail!(format!(
                    "invalid config: target branch {} is not a valid regex",
                    &target.name
                ))
            }
        }
    }

    Ok(())
}

fn create_is_valid(config: &Config) -> Result<()> {
    let invalid_creates = config
        .branch_types
        .iter()
        .filter(|x| {
            if x.name == "customer-release" {
                x.create.match_indices("{RELEASE}").count() != 1
            } else {
                x.create.match_indices(BRANCH_NAME_PLACEHOLDER).count() != 1
            }
        })
        .map(|x| format!("{}: {}", x.name, x.create))
        .collect::<Vec<String>>();

    if !invalid_creates.is_empty() {
        bail!(
            "These branch_types have invalid 'create' templates:\n{}",
            invalid_creates.join("\n")
        )
    }

    Ok(())
}
