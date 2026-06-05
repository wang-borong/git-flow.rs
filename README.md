# gitflow.rs

Extensible gitflow written in Rust.

**Extensible:** Customize the workflow that suits your preferences.

**Config-driven:** Standardize team protocols with TOML configuration.

**Customer branches:** Built-in support for multi-customer customization workflows.

## Installation

Download from [GitHub Releases](https://github.com/niuiic/gitflow.rs/releases).

## Quick Start

```sh
# Initialize config interactively
gitflow init

# Start a feature
gitflow feature start my-feature

# Finish the feature (squash merge to main, delete branch)
gitflow feature finish my-feature

# Or use the shorthand for the current branch
gitflow finish
```

## Commands

```
Usage: gitflow [OPTIONS] <COMMAND>

Commands:
  feature   Manage feature branches
  release   Manage release branches
  hotfix    Manage hotfix branches
  custom    Manage customer branches (long-lived)
  general   Manage generalize branches (定制转通用)
  finish    Finish current branch (shorthand)
  update    Update current branch (shorthand)
  delete    Delete current branch (shorthand)
  rename    Rename current branch (shorthand)
  publish   Publish current branch (shorthand)
  continue  continue after resolving conflicts
  abort     abort current operation and restore previous state
  check     check config
  complete  generate shell completion
  init      initialize gitflow config interactively
  list      list available branch types
  overview  display a comprehensive overview of repository status
```

### Options
```
  -c, --config <FILE>  path to config file
      --dry-run        show what would be done without executing
      --verbose        show detailed git operations
  -h, --help           print help
  -V, --version        print version
```

### Feature / Hotfix / Release / General Branches

The standard syntax for these branches is `gitflow <type> <action>`.

```sh
# Start a branch
gitflow feature start my-feature

# Start from a customer branch
gitflow hotfix start my-fix --customer aliyun

# Standard finish (merge to configured targets, delete branch)
gitflow feature finish my-feature

# Keep branch after finish
gitflow feature finish my-feature --keep

# Create a tag after finish (often used with release)
gitflow release finish v1.0.0 --tag

# Squash merge (override configured strategy)
gitflow feature finish my-feature --squash

# Push target branches to remote
gitflow feature finish my-feature --push

# Finish a general task and auto-revert picked commits on the customer branch
gitflow general finish --cleanup-customer
```

### Customer Branches (Long-lived)

```sh
# Create a customer branch from main
gitflow custom start aliyun

# Create and push to remote
gitflow custom start aliyun --push

# Sync main changes into a customer branch
gitflow custom sync aliyun

# Sync using rebase instead of merge
gitflow custom sync aliyun --rebase

# Sync all customer branches
gitflow custom sync all

# Sync all customer branches using rebase
gitflow custom sync all --rebase

# List all customer branches with their status
gitflow custom list
```

### Shell Completion

```sh
# Generate completion script
gitflow complete bash >> ~/.bashrc
gitflow complete zsh >> ~/.zshrc
gitflow complete fish > ~/.config/fish/completions/gitflow.fish
```

## Configuration

Config file locations (in priority order):

1. Explicit path: `gitflow -c /path/to/config.toml <command>`
2. Local: `<GitRoot>/.gitflow.toml`
3. Global: `~/.config/gitflow/config.toml` (Linux/macOS) or `%APPDATA%/gitflow/config.toml` (Windows)

### YQM Format

The `[branches]`/`[commands]`/`[merge]`/`[hooks]` format supports customer branches and the yqm workflow natively.

```toml
[branches]
main = "main"
customer = { prefix = "customer/" }
generalize = { prefix = "generalize/" }
release = { prefix = "release/" }

[commands]
disable = []

[merge]
default_strategy = "squash"           # feature/hotfix → main
release_strategy = "merge"            # release → main
customer_sync_strategy = "merge"      # main → customer

[hooks]
post_start = "git push origin {BRANCH}:{BRANCH}"
post_finish = "ci/deploy.sh"
post_tag = "ci/archive.sh {TAG}"
post_customer_sync = "ci/sync-customer.sh {CUSTOMER}"
```

### Available Strategies

| Strategy | Description |
|---|---|
| `merge` | Standard git merge |
| `rebase` | Rebase onto target branch |
| `cherry-pick` | Cherry-pick individual commits |
| `squash` | Squash merge (single commit) |

### Available Hooks

| Hook | When |
|---|---|
| `before_start` / `after_start` | Before/after branch creation |
| `before_finish` / `after_finish` | Before/after merge and deletion |
| `before_drop` / `after_drop` | Before/after branch drop |
| `before_publish` / `after_publish` | Before/after push to remote |
| `before_rebase` / `after_rebase` | Before/after rebase operation |

### Safety Rules

When using the yqm config format, the following rules are enforced:

- Only `feature/*`, `hotfix/*`, `generalize/*`, `release/*` can merge to `main`
- `feature/customer-*` and `hotfix/customer-*` are blocked from merging to `main`
- `customer/*` long-lived branches cannot merge directly to `main`
- Release branches must use `merge` strategy (not `squash`)

## License

MIT
