use std::{fs::File, io::Read, path::PathBuf};

use anyhow::{bail, Context, Result};

use super::{definition, path, validate::validate_config, yqm};
use crate::git::Git;

#[cfg(test)]
mod test;

pub fn read_config(config_path: Option<PathBuf>) -> Result<definition::Config> {
    // -- get path --
    let config_path_list = match config_path {
        Some(config_path_v) => vec![config_path_v],
        None => path::get_config_path_list().context("unable to get default config path")?,
    };

    // -- read file --
    let mut config_file = None;
    for config_path in config_path_list {
        if let Ok(config_file_v) = File::open(config_path) {
            config_file = Some(config_file_v);
            break;
        }
    }
    if config_file.is_none() {
        bail!("config file is not found");
    }
    let mut text = String::new();
    config_file.unwrap().read_to_string(&mut text)?;

    // -- detect format and parse --
    let mut config = if yqm::is_yqm_format(&text) {
        yqm::parse_yqm_config(&text).context("unable to parse yqm config")?
    } else {
        toml::from_str::<definition::Config>(&text).context("unable to parse config")?
    };

    // -- resolve base override --
    if !config.allow_non_main_base {
        // Resolve primary base branch
        let main_branch = if let Some(ref base) = config.base_branch {
            base.clone()
        } else {
            // Auto-detect master or main branch
            if let Ok(git) = Git::open() {
                if let Ok(branches) = git.get_local_branches() {
                    if branches.iter().any(|b| b == "master")
                        && !branches.iter().any(|b| b == "main")
                    {
                        "master".to_string()
                    } else {
                        "main".to_string()
                    }
                } else {
                    "main".to_string()
                }
            } else {
                "main".to_string()
            }
        };

        for bt in &mut config.branch_types {
            let old_from = bt.from.clone();
            let should_override = if let Some(ref base) = config.base_branch {
                bt.from != *base
            } else {
                bt.from != "main" && bt.from != "master"
            };

            if should_override {
                bt.from = main_branch.clone();
            }

            // Also redirect matching targets
            for target in &mut bt.to {
                if target.name == old_from {
                    target.name = main_branch.clone();
                }
            }
        }
    }

    // -- validate --
    match validate_config(&config) {
        Ok(_) => Ok(config),
        Err(err) => Err(err.context("config is invalid")),
    }
}
