use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{bail, Result};

use crate::echo::Echo;

pub fn init_config() {
    match interactive_init() {
        Err(err) => Echo::error(err.to_string()),
        Ok(path) => Echo::success(&format!("config written to {}", path.display())),
    }
}

fn interactive_init() -> Result<PathBuf> {
    println!("git-flow interactive configuration\n");

    // -- select base template --
    println!("Select a template:");
    println!("[1] Standard gitflow (feature/release/hotfix from dev)");
    println!("[2] GitHub flow (feature from main)");
    println!("[3] Custom (build from scratch)");
    print!("Choice (1-3): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice = input.trim();

    let toml_content = match choice {
        "1" => standard_template(),
        "2" => github_flow_template(),
        "3" => custom_template()?,
        _ => bail!("invalid choice"),
    };

    // -- select output path --
    println!("\nWhere to save the config?");
    println!("[1] Local (.git-flow.toml in repo root)");
    println!("[2] Global (~/.config/git-flow/config.toml)");
    print!("Choice (1-2): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let path = match input.trim() {
        "1" => {
            let repo = git2::Repository::open_from_env()?;
            let workdir = repo.workdir().unwrap_or(repo.path());
            workdir.join(".git-flow.toml")
        }
        "2" => {
            let config_dir = dirs_config_dir()?;
            std::fs::create_dir_all(&config_dir)?;
            config_dir.join("config.toml")
        }
        _ => bail!("invalid choice"),
    };

    // -- check if file exists --
    if path.exists() {
        print!("File already exists. Overwrite? (y/N): ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() != "y" {
            bail!("aborted");
        }
    }

    // -- write config --
    let mut file = std::fs::File::create(&path)?;
    file.write_all(toml_content.as_bytes())?;

    Ok(path)
}

fn standard_template() -> String {
    r#"[[branch_types]]
name = "feature"
create = "feature/{NAME}"
from = "dev"
to = [{ name = "dev", strategy = "merge" }]
remote = "origin"
after_start = { command = "git", args = ["push", "origin", "feature/{NAME}:feature/{NAME}"] }
after_finish = { command = "git", args = ["push", "origin", "--delete", "feature/{NAME}"] }

[[branch_types]]
name = "release"
create = "release/{NAME}"
from = "dev"
to = [{ name = "main", strategy = "merge", tag = true }]
remote = "origin"
tag_pattern = "v{NAME}"

[[branch_types]]
name = "hotfix"
create = "hotfix/{NAME}"
from = "main"
to = [
  { name = "main", strategy = "merge", tag = true },
  { name = "dev", strategy = "merge" },
]
remote = "origin"
tag_pattern = "v{NAME}"
"#
    .to_string()
}

fn github_flow_template() -> String {
    r#"[[branch_types]]
name = "feature"
create = "feature/{NAME}"
from = "main"
to = [{ name = "main", strategy = "merge" }]
remote = "origin"
"#
    .to_string()
}

fn custom_template() -> Result<String> {
    let mut branches = Vec::new();

    loop {
        print!("\nBranch type name (empty to finish): ");
        io::stdout().flush()?;
        let mut name = String::new();
        io::stdin().read_line(&mut name)?;
        let name = name.trim().to_string();
        if name.is_empty() {
            break;
        }

        print!("Create pattern (e.g. {}/{{NAME}}): ", name);
        io::stdout().flush()?;
        let mut create = String::new();
        io::stdin().read_line(&mut create)?;
        let create = create.trim().to_string();

        print!("Source branch (e.g. dev): ");
        io::stdout().flush()?;
        let mut from = String::new();
        io::stdin().read_line(&mut from)?;
        let from = from.trim().to_string();

        print!("Target branches (comma-separated, e.g. dev,main): ");
        io::stdout().flush()?;
        let mut to_input = String::new();
        io::stdin().read_line(&mut to_input)?;
        let to_branches: Vec<String> = to_input
            .trim()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        print!("Strategy for targets (merge/rebase/cherry-pick/squash) [merge]: ");
        io::stdout().flush()?;
        let mut strategy = String::new();
        io::stdin().read_line(&mut strategy)?;
        let strategy = strategy.trim();
        let strategy = if strategy.is_empty() {
            "merge"
        } else {
            strategy
        };

        print!("Remote name (e.g. origin, empty to skip): ");
        io::stdout().flush()?;
        let mut remote = String::new();
        io::stdin().read_line(&mut remote)?;
        let remote = remote.trim().to_string();

        let to_str = to_branches
            .iter()
            .map(|b| format!("  {{ name = \"{}\", strategy = \"{}\" }}", b, strategy))
            .collect::<Vec<_>>()
            .join(",\n");

        let remote_line = if remote.is_empty() {
            String::new()
        } else {
            format!("remote = \"{}\"\n", remote)
        };

        branches.push(format!(
            "[[branch_types]]\nname = \"{}\"\ncreate = \"{}\"\nfrom = \"{}\"\nto = [\n{}\n]\n{}",
            name, create, from, to_str, remote_line
        ));

        println!("Added branch type '{}'", name);
    }

    if branches.is_empty() {
        bail!("no branch types defined");
    }

    Ok(branches.join("\n") + "\n")
}

fn dirs_config_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        bail!("cannot determine home directory");
    }
    Ok(PathBuf::from(home).join(".config").join("git-flow"))
}
