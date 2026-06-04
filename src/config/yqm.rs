use anyhow::{bail, Result};
use serde::Deserialize;

use super::definition::{BranchType, Command, Config, Strategy, TargetBranch};

/// New yqm config format: `[branches]`/`[commands]`/`[merge]`/`[hooks]`
#[derive(Debug, Deserialize, Clone)]
pub struct YqmConfig {
    pub branches: BranchesConfig,
    #[serde(default)]
    pub commands: CommandsConfig,
    #[serde(default)]
    pub merge: MergeConfig,
    #[serde(default)]
    pub hooks: HooksConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BranchesConfig {
    pub main: String,
    pub customer: PrefixConfig,
    pub generalize: PrefixConfig,
    /// release is optional (cloud repos don't need it)
    pub release: Option<PrefixConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PrefixConfig {
    pub prefix: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct CommandsConfig {
    #[serde(default)]
    pub disable: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MergeConfig {
    #[serde(default = "default_squash")]
    pub default_strategy: String,
    #[serde(default = "default_merge")]
    pub release_strategy: String,
    #[serde(default = "default_merge")]
    #[allow(dead_code)]
    pub customer_sync_strategy: String,
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            default_strategy: default_squash(),
            release_strategy: default_merge(),
            customer_sync_strategy: default_merge(),
        }
    }
}

fn default_squash() -> String {
    "squash".to_string()
}

fn default_merge() -> String {
    "merge".to_string()
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct HooksConfig {
    pub post_start: Option<String>,
    pub post_finish: Option<String>,
    #[allow(dead_code)]
    pub post_customer_sync: Option<String>,
    pub post_tag: Option<String>,
}

/// Check if TOML content is in yqm format (has `[branches]` section)
pub fn is_yqm_format(toml_content: &str) -> bool {
    // Quick check: look for `[branches]` section header
    toml_content.contains("[branches]")
}

/// Parse yqm config and convert to the standard Config (branch_types)
pub fn parse_yqm_config(toml_content: &str) -> Result<Config> {
    let yqm: YqmConfig = toml::from_str(toml_content)
        .map_err(|e| anyhow::anyhow!("failed to parse yqm config: {}", e))?;

    let mut branch_types = Vec::new();

    let main = &yqm.branches.main;
    let customer_prefix = &yqm.branches.customer.prefix;
    let _generalize_prefix = &yqm.branches.generalize.prefix;

    let default_strategy = parse_strategy(&yqm.merge.default_strategy)?;
    let release_strategy = parse_strategy(&yqm.merge.release_strategy)?;

    let is_disabled = |name: &str| yqm.commands.disable.iter().any(|d| d == name);

    // -- feature (from main, squash→main) --
    if !is_disabled("feature") {
        branch_types.push(BranchType {
            name: "feature".to_string(),
            create: "feature/{{NAME}}".to_string(),
            from: main.clone(),
            to: vec![TargetBranch {
                name: main.clone(),
                strategy: default_strategy.clone(),
                push: None,
                tag: None,
            }],
            remote: None,
            tag_pattern: None,
            before_start: None,
            after_start: hook_cmd(&yqm.hooks.post_start),
            before_finish: None,
            after_finish: hook_cmd(&yqm.hooks.post_finish),
            before_drop: None,
            after_drop: None,
            before_publish: None,
            after_publish: None,
            before_rebase: None,
            after_rebase: None,
        });
    }

    // -- customer feature (from customer/x, squash→customer/x) --
    branch_types.push(BranchType {
        name: "customer-feature".to_string(),
        create: "feature/customer-{{NAME}}/{{FEATURE}}".to_string(),
        from: format!("{}{{NAME}}", customer_prefix),
        to: vec![TargetBranch {
            name: format!("{}{{NAME}}", customer_prefix),
            strategy: default_strategy.clone(),
            push: None,
            tag: None,
        }],
        remote: None,
        tag_pattern: None,
        before_start: None,
        after_start: hook_cmd(&yqm.hooks.post_start),
        before_finish: None,
        after_finish: hook_cmd(&yqm.hooks.post_finish),
        before_drop: None,
        after_drop: None,
        before_publish: None,
        after_publish: None,
        before_rebase: None,
        after_rebase: None,
    });

    // -- hotfix (from main, squash→main) --
    if !is_disabled("hotfix") {
        branch_types.push(BranchType {
            name: "hotfix".to_string(),
            create: "hotfix/{{NAME}}".to_string(),
            from: main.clone(),
            to: vec![TargetBranch {
                name: main.clone(),
                strategy: default_strategy.clone(),
                push: None,
                tag: None,
            }],
            remote: None,
            tag_pattern: None,
            before_start: None,
            after_start: hook_cmd(&yqm.hooks.post_start),
            before_finish: None,
            after_finish: hook_cmd(&yqm.hooks.post_finish),
            before_drop: None,
            after_drop: None,
            before_publish: None,
            after_publish: None,
            before_rebase: None,
            after_rebase: None,
        });
    }

    // -- customer hotfix (from customer/x, squash→customer/x) --
    branch_types.push(BranchType {
        name: "customer-hotfix".to_string(),
        create: "hotfix/customer-{{NAME}}/{{FIX}}".to_string(),
        from: format!("{}{{NAME}}", customer_prefix),
        to: vec![TargetBranch {
            name: format!("{}{{NAME}}", customer_prefix),
            strategy: default_strategy.clone(),
            push: None,
            tag: None,
        }],
        remote: None,
        tag_pattern: None,
        before_start: None,
        after_start: hook_cmd(&yqm.hooks.post_start),
        before_finish: None,
        after_finish: hook_cmd(&yqm.hooks.post_finish),
        before_drop: None,
        after_drop: None,
        before_publish: None,
        after_publish: None,
        before_rebase: None,
        after_rebase: None,
    });

    // -- generalize (from customer/x, squash→main) --
    if !is_disabled("generalize") {
        branch_types.push(BranchType {
            name: "generalize".to_string(),
            create: "feature/{NAME}".to_string(),
            from: format!("{}{{CUSTOMER}}", customer_prefix),
            to: vec![TargetBranch {
                name: main.clone(),
                strategy: default_strategy.clone(),
                push: None,
                tag: None,
            }],
            remote: None,
            tag_pattern: None,
            before_start: None,
            after_start: None,
            before_finish: None,
            after_finish: hook_cmd(&yqm.hooks.post_finish),
            before_drop: None,
            after_drop: None,
            before_publish: None,
            after_publish: None,
            before_rebase: None,
            after_rebase: None,
        });
    }

    // -- release (from main, merge→main, tag) --
    if !is_disabled("release") {
        if let Some(ref release_config) = yqm.branches.release {
            // General release
            branch_types.push(BranchType {
                name: "release".to_string(),
                create: format!("{}{{NAME}}", release_config.prefix),
                from: main.clone(),
                to: vec![TargetBranch {
                    name: main.clone(),
                    strategy: release_strategy.clone(),
                    push: None,
                    tag: Some(true),
                }],
                remote: None,
                tag_pattern: Some("{{NAME}}".to_string()),
                before_start: None,
                after_start: None,
                before_finish: None,
                after_finish: hook_cmd(&yqm.hooks.post_finish),
                before_drop: None,
                after_drop: None,
                before_publish: None,
                after_publish: None,
                before_rebase: None,
                after_rebase: hook_cmd(&yqm.hooks.post_tag),
            });

            // Customer release (from customer/x, merge→customer/x, tag)
            branch_types.push(BranchType {
                name: "customer-release".to_string(),
                create: format!("{}{{NAME}}", release_config.prefix),
                from: format!("{}{{CUSTOMER}}", customer_prefix),
                to: vec![TargetBranch {
                    name: format!("{}{{CUSTOMER}}", customer_prefix),
                    strategy: release_strategy.clone(),
                    push: None,
                    tag: Some(true),
                }],
                remote: None,
                tag_pattern: Some("{{NAME}}".to_string()),
                before_start: None,
                after_start: None,
                before_finish: None,
                after_finish: hook_cmd(&yqm.hooks.post_finish),
                before_drop: None,
                after_drop: None,
                before_publish: None,
                after_publish: None,
                before_rebase: None,
                after_rebase: hook_cmd(&yqm.hooks.post_tag),
            });
        }
    }

    Ok(Config { branch_types })
}

fn parse_strategy(s: &str) -> Result<Strategy> {
    match s {
        "merge" => Ok(Strategy::Merge),
        "rebase" => Ok(Strategy::Rebase),
        "cherry-pick" => Ok(Strategy::CherryPick),
        "squash" => Ok(Strategy::Squash),
        _ => bail!("unknown strategy: {}", s),
    }
}

fn hook_cmd(cmd: &Option<String>) -> Option<Command> {
    cmd.as_ref().map(|c| Command {
        command: "sh".to_string(),
        args: vec!["-c".to_string(), c.clone()],
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_cloud_config() {
        let toml = r#"
[branches]
main = "main"
customer = { prefix = "customer/" }
generalize = { prefix = "generalize/" }

[commands]
disable = ["release"]

[merge]
default_strategy = "squash"
customer_sync_strategy = "merge"
"#;
        let config = parse_yqm_config(toml).unwrap();
        assert!(config.branch_types.iter().any(|b| b.name == "feature"));
        assert!(config
            .branch_types
            .iter()
            .any(|b| b.name == "customer-feature"));
        assert!(config.branch_types.iter().any(|b| b.name == "hotfix"));
        assert!(config
            .branch_types
            .iter()
            .any(|b| b.name == "customer-hotfix"));
        assert!(config.branch_types.iter().any(|b| b.name == "generalize"));
        assert!(!config.branch_types.iter().any(|b| b.name == "release"));
        assert!(!config
            .branch_types
            .iter()
            .any(|b| b.name == "customer-release"));
    }

    #[test]
    fn test_parse_embedded_config() {
        let toml = r#"
[branches]
main = "main"
customer = { prefix = "customer/" }
generalize = { prefix = "generalize/" }
release = { prefix = "release/" }

[merge]
default_strategy = "squash"
release_strategy = "merge"
customer_sync_strategy = "merge"
"#;
        let config = parse_yqm_config(toml).unwrap();
        assert!(config.branch_types.iter().any(|b| b.name == "release"));
        assert!(config
            .branch_types
            .iter()
            .any(|b| b.name == "customer-release"));

        // Release should use merge strategy, not squash
        let release = config
            .branch_types
            .iter()
            .find(|b| b.name == "release")
            .unwrap();
        assert!(matches!(release.to[0].strategy, Strategy::Merge));
        assert_eq!(release.to[0].tag, Some(true));
    }

    #[test]
    fn test_is_yqm_format() {
        assert!(is_yqm_format("[branches]\nmain = \"main\""));
        assert!(!is_yqm_format("[[branch_types]]\nname = \"feature\""));
    }

    #[test]
    fn test_customer_feature_create_pattern() {
        let toml = r#"
[branches]
main = "main"
customer = { prefix = "customer/" }
generalize = { prefix = "generalize/" }

[commands]
disable = ["release"]
"#;
        let config = parse_yqm_config(toml).unwrap();
        let cf = config
            .branch_types
            .iter()
            .find(|b| b.name == "customer-feature")
            .unwrap();
        // create pattern should be "feature/customer-{NAME}/{FEATURE}"
        assert!(cf.create.contains("customer-"));
        // from should be "customer/{NAME}"
        assert!(cf.from.starts_with("customer/"));
    }
}
