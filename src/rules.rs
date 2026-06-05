use anyhow::{bail, Result};

use crate::config::definition::{BranchType, Strategy};
use crate::echo::Echo;

/// Determine if a branch prefix represents a customer-scoped long-lived branch.
/// By convention, customer branches have a `create` pattern of "customer/{NAME}".
fn is_customer_branch(branch_name: &str, branch_types: &[BranchType]) -> bool {
    branch_types.iter().any(|bt| {
        // A "customer" branch type is one where `create` starts with "customer/"
        // or `name` is "customer"
        (bt.name == "customer" || bt.create.starts_with("customer/")) && {
            let prefix = bt.create.split('{').next().unwrap_or("customer/");
            branch_name.starts_with(prefix)
        }
    })
}

/// Determine if a branch is a customer-scoped feature/hotfix/general branch.
/// These are branches that exist for a specific customer and should NOT merge to main.
/// Derived from config: branch types whose `create` pattern contains a customer prefix
/// AND whose `from` is a customer branch (not main).
fn is_customer_scoped_branch(branch_name: &str, branch_types: &[BranchType]) -> bool {
    branch_types.iter().any(|bt| {
        // Customer-scoped branch types have `from` pointing to a customer branch
        // AND their `create` pattern reflects a customer prefix
        bt.from.starts_with("customer/")
            || (bt.create.contains("customer-") && {
                let prefix = bt.create.split('{').next().unwrap_or("");
                !prefix.is_empty() && branch_name.starts_with(prefix)
            })
    })
}

/// Build a list of branch prefixes that are allowed to merge to the main branch.
/// These are all branch types whose `from` equals the main_branch (or a variation).
/// ISSUE-R2: Previously hardcoded; now derived from config.
fn allowed_prefixes_for_main(branch_types: &[BranchType], main_branch: &str) -> Vec<String> {
    branch_types
        .iter()
        .filter(|bt| {
            // A branch type is allowed to merge to main if:
            // 1. It originates from main (from == main_branch) OR is a standard release/hotfix/general/generalize branch
            // 2. It is not a long-lived customer branch type
            (bt.from == main_branch
                || bt.name == "release"
                || bt.name == "hotfix"
                || bt.name == "general"
                || bt.name == "generalize")
                && bt.name != "customer"
                && !bt.create.starts_with("customer/")
        })
        .map(|bt| {
            // Extract the prefix from the `create` pattern (before the first placeholder brace)
            bt.create.split('{').next().unwrap_or("").to_string()
        })
        .filter(|p| !p.is_empty())
        .collect()
}

/// Validate that a branch is allowed to merge into the target branch.
/// Safety rules are now derived from the branch_types configuration.
///
/// Core rules:
/// - Customer long-lived branches (customer/*) cannot merge to main
/// - Customer-scoped branches (feature/customer-*, hotfix/customer-*, etc.) cannot merge to main
/// - Only branches originating from main can merge to main
/// - Customer branches cannot merge to other customer branches
pub fn validate_merge_allowed(
    branch_name: &str,
    target_branch: &str,
    main_branch: &str,
    // ISSUE-R1/R2: Accept branch_types for dynamic rule derivation
    branch_types: &[BranchType],
) -> Result<()> {
    if target_branch == main_branch {
        // Customer long-lived branches cannot merge to main
        if is_customer_branch(branch_name, branch_types) {
            bail!(
                "safety rule: customer branch '{}' is not allowed to merge directly to '{}'. \
                 Use a general/ branch to contribute customer changes to main.",
                branch_name,
                main_branch
            );
        }

        // Customer-scoped feature/hotfix/general branches cannot merge to main directly
        // ISSUE-R1 fix: This is now derived from config, not hardcoded "feature/customer-"
        let customer_scoped = is_customer_scoped_branch(branch_name, branch_types);
        if customer_scoped {
            bail!(
                "safety rule: customer-specific branch '{}' is not allowed to merge to '{}'. \
                 It should merge to its corresponding customer/ branch instead.",
                branch_name,
                main_branch
            );
        }

        // ISSUE-R2 fix: Derive allowed prefixes from config instead of hardcoding
        let allowed_prefixes = allowed_prefixes_for_main(branch_types, main_branch);
        if !allowed_prefixes.is_empty()
            && !allowed_prefixes
                .iter()
                .any(|p| branch_name.starts_with(p.as_str()))
        {
            bail!(
                "safety rule: branch '{}' is not allowed to merge to '{}'. \
                 Allowed branch types: {}.",
                branch_name,
                main_branch,
                allowed_prefixes.join(", ")
            );
        }

        Ok(())
    } else if target_branch.starts_with("customer/") {
        let target_customer = target_branch.strip_prefix("customer/").unwrap();

        // Customer long-lived branches cannot merge to other customer branches
        if is_customer_branch(branch_name, branch_types) {
            bail!("safety rule: customer branch '{}' is not allowed to merge to another customer branch '{}'.", branch_name, target_branch);
        }

        // Allowed to merge to customer/X:
        // - feature/customer-X/*
        // - hotfix/customer-X/*
        // - general/customer-X/* or generalize/customer-X/*
        let allowed_prefixes = [
            format!("feature/customer-{}/", target_customer),
            format!("hotfix/customer-{}/", target_customer),
            format!("general/customer-{}/", target_customer),
            format!("generalize/customer-{}/", target_customer),
        ];

        if !allowed_prefixes.iter().any(|p| branch_name.starts_with(p)) {
            bail!(
                "safety rule: branch '{}' is not allowed to merge to customer branch '{}'. \
                 Only its corresponding customer-specific feature, hotfix, or general branches can merge here.",
                branch_name,
                target_branch
            );
        }

        Ok(())
    } else {
        // Forbidden to merge customer branch to anywhere else
        if is_customer_branch(branch_name, branch_types) {
            bail!(
                "safety rule: customer branch '{}' is not allowed to merge to '{}'.",
                branch_name,
                target_branch
            );
        }
        Ok(())
    }
}

/// Enforce merge strategy rules:
/// - release/* → main: must use merge (not squash)
/// - Others → main: squash is recommended (but not enforced)
pub fn validate_strategy(
    branch_name: &str,
    target_branch: &str,
    strategy: &Strategy,
    main_branch: &str,
) -> Result<()> {
    if target_branch != main_branch {
        return Ok(());
    }

    // Release branches must use merge, not squash
    if branch_name.starts_with("release/") {
        if matches!(strategy, Strategy::Squash) {
            bail!(
                "safety rule: release branch '{}' must use 'merge' strategy, not 'squash'. \
                 Release branches need to preserve verification history.",
                branch_name
            );
        }
        return Ok(());
    }

    // Other branches merging to main should use squash
    if !matches!(strategy, Strategy::Squash) {
        Echo::warning(format!(
            "branch '{}' merging to '{}' uses {:?} strategy instead of recommended 'squash'",
            branch_name, target_branch, strategy
        ));
    }

    Ok(())
}
