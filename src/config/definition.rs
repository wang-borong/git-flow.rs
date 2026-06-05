use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub branch_types: Vec<BranchType>,
    #[serde(default)]
    pub allow_non_main_base: bool,
    pub base_branch: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BranchType {
    pub name: String,
    pub create: String,
    pub from: String,
    pub to: Vec<TargetBranch>,
    /// Default remote name for publish/push operations (e.g. "origin")
    pub remote: Option<String>,
    /// Tag name pattern for finish --tag (e.g. "v{NAME}")
    pub tag_pattern: Option<String>,
    pub before_start: Option<Command>,
    pub after_start: Option<Command>,
    pub before_finish: Option<Command>,
    pub after_finish: Option<Command>,
    pub before_drop: Option<Command>,
    pub after_drop: Option<Command>,
    pub before_publish: Option<Command>,
    pub after_publish: Option<Command>,
    pub before_rebase: Option<Command>,
    pub after_rebase: Option<Command>,
}

impl BranchType {
    pub fn resolve_customer(&mut self, branch_name: &str, customer_opt: Option<&str>) {
        let mut pattern = self.create.clone();
        pattern = pattern.replace("{{CUSTOMER}}", "(?P<customer>[^/]+)");
        pattern = pattern.replace("{CUSTOMER}", "(?P<customer>[^/]+)");
        pattern = pattern.replace("{{NAME}}", "(?P<name>[^/]+)");
        pattern = pattern.replace("{NAME}", "(?P<name>[^/]+)");
        pattern = pattern.replace("{{FEATURE}}", "(?P<feature>[^/]+)");
        pattern = pattern.replace("{FEATURE}", "(?P<feature>[^/]+)");
        pattern = pattern.replace("{{FIX}}", "(?P<fix>[^/]+)");
        pattern = pattern.replace("{FIX}", "(?P<fix>[^/]+)");

        let re = match regex::Regex::new(&format!("^{}$", pattern)) {
            Ok(r) => r,
            Err(_) => return,
        };

        let extracted_customer = if let Some(caps) = re.captures(branch_name) {
            if caps.name("customer").is_some() {
                caps.name("customer").map(|m| m.as_str().to_string())
            } else if self.name.starts_with("customer-") && caps.name("name").is_some() {
                caps.name("name").map(|m| m.as_str().to_string())
            } else {
                None
            }
        } else {
            None
        };

        let cust_val = customer_opt.map(|s| s.to_string()).or(extracted_customer);
        if let Some(ref cust) = cust_val {
            let customer_branch = format!("customer/{}", cust);
            self.from = customer_branch.clone();
            for to_branch in &mut self.to {
                if customer_opt.is_some() || self.name.starts_with("customer-") {
                    to_branch.name = customer_branch.clone();
                } else {
                    to_branch.name = to_branch
                        .name
                        .replace("{{CUSTOMER}}", cust)
                        .replace("{CUSTOMER}", cust)
                        .replace("{{NAME}}", cust)
                        .replace("{NAME}", cust);
                }
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TargetBranch {
    pub name: String,
    pub strategy: Strategy,
    /// Auto-push to remote after merge into this target branch
    pub push: Option<bool>,
    /// Auto-tag after merge into this target branch
    pub tag: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum Strategy {
    #[serde(rename = "merge")]
    Merge,
    #[serde(rename = "rebase")]
    Rebase,
    #[serde(rename = "cherry-pick")]
    CherryPick,
    #[serde(rename = "squash")]
    Squash,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Command {
    pub command: String,
    pub args: Vec<String>,
}

pub const BRANCH_NAME_PLACEHOLDER: &str = "{NAME}";
