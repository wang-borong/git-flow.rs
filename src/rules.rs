use anyhow::{bail, Result};

use crate::config::definition::Strategy;
use crate::echo::Echo;

/// Validate that a branch is allowed to merge into the target branch.
/// Safety rules:
/// - Only feature/*, hotfix/*, generalize/*, release/* can merge to main
/// - feature/customer-*, hotfix/customer-* are forbidden from merging to main
/// - customer/* long-lived branches cannot merge to main
pub fn validate_merge_allowed(
    branch_name: &str,
    target_branch: &str,
    main_branch: &str,
) -> Result<()> {
    if target_branch == main_branch {
        // Customer long-lived branches cannot merge to main
        if branch_name.starts_with("customer/") {
            bail!(
                "safety rule: customer branch '{}' is not allowed to merge directly to '{}'. \
                 Use a general/ branch to contribute customer changes to main.",
                branch_name,
                main_branch
            );
        }

        // Customer-specific feature/hotfix/general branches cannot merge to main directly if they are customer-specific
        if branch_name.starts_with("feature/customer-")
            || branch_name.starts_with("hotfix/customer-")
        {
            bail!(
                "safety rule: customer-specific branch '{}' is not allowed to merge to '{}'. \
                 It should merge to its corresponding customer/ branch instead.",
                branch_name,
                main_branch
            );
        }

        // Only feature/*, hotfix/*, general/*, generalize/*, release/* are allowed to merge to main
        let allowed_prefixes = ["feature/", "hotfix/", "general/", "generalize/", "release/"];
        if !allowed_prefixes.iter().any(|p| branch_name.starts_with(p)) {
            bail!(
                "safety rule: branch '{}' is not allowed to merge to '{}'. \
                 Only feature/*, hotfix/*, general/*, release/* branches can merge to main.",
                branch_name,
                main_branch
            );
        }

        Ok(())
    } else if target_branch.starts_with("customer/") {
        let target_customer = target_branch.strip_prefix("customer/").unwrap();

        // Customer long-lived branches cannot merge to other customer branches
        if branch_name.starts_with("customer/") {
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
        if branch_name.starts_with("customer/") {
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
/// - feature/*, hotfix/*, generalize/* → main: must use squash
/// - release/* → main: must use merge (not squash)
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
