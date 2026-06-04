# git-flow.rs

Extensible git flow written in Rust.

**Extensible:** Customize the workflow that suits your preferences.

**Config-driven:** Standardize team protocols with TOML configuration.

**Customer branches:** Built-in support for multi-customer customization workflows.

## Installation

```sh
cargo install git-flow-rs
```

Or download from [GitHub Releases](https://github.com/niuiic/git-flow.rs/releases).

## Quick Start

```sh
# Initialize config interactively
git flow init

# Start a feature
git flow start my-feature feature
# or: git flow start feature/my-feature

# Finish the feature (squash merge to main, delete branch)
git flow finish feature/my-feature
```

## Commands

```
Usage: git-flow [OPTIONS] <COMMAND>

Commands:
  start      start a task
  finish     finish a task
  drop       drop a task
  track      track a task
  sync       sync branches between local/remote
  list       list available branch types
  check      validate a config file
  complete   generate shell completion (bash/zsh/fish/elvish/powershell)
  init       initialize git-flow config interactively
  publish    publish current branch to remote
  rebase     rebase current branch onto its source branch
  continue   continue after resolving conflicts
  abort      abort current operation and restore previous state
  customer   manage customer branches (create/sync/list)

Options:
  -c, --config <FILE>  path to config file
      --dry-run        show what would be done without executing
      --verbose        show detailed git operations
  -h, --help           print help
  -V, --version        print version
```

### start

```sh
# Start from a configured branch type
git flow start my-feature feature

# Fetch source branch before creating
git flow start my-feature feature --fetch

# Start from a customer branch
git flow start my-fix hotfix --customer aliyun
```

### finish

```sh
# Standard finish (merge to configured targets, delete branch)
git flow finish feature/my-feature

# Keep branch after finish
git flow finish feature/my-feature --keep

# Create a tag after finish
git flow finish release/v1.0.0 --tag

# Squash merge (override configured strategy)
git flow finish feature/my-feature --squash

# Push target branches to remote
git flow finish feature/my-feature --push

# Fetch before finish
git flow finish feature/my-feature --fetch

# Finish to a customer branch instead of main
git flow finish feature/my-feature --customer aliyun
```

### customer

```sh
# Create a customer branch from main
git flow customer create aliyun

# Create and push to remote
git flow customer create aliyun --push

# Sync main changes into a customer branch
git flow customer sync aliyun

# Sync and push
git flow customer sync aliyun --push

# Sync all customer branches
git flow customer sync all

# List all customer branches with their status
git flow customer list
```

### Shell Completion

```sh
# Generate completion script
git flow complete bash >> ~/.bashrc
git flow complete zsh >> ~/.zshrc
git flow complete fish > ~/.config/fish/completions/git-flow.fish
```

## Configuration

Config file locations (in priority order):

1. Explicit path: `git flow -c /path/to/config.toml <command>`
2. Local: `<GitRoot>/.git-flow.toml`
3. Global: `~/.config/git-flow/config.toml` (Linux/macOS) or `%APPDATA%/git-flow/config.toml` (Windows)

### New Format (yqm)

The new `[branches]`/`[commands]`/`[merge]`/`[hooks]` format supports customer branches and the yqm workflow.

```toml
[branches]
main = "main"
customer = { prefix = "customer/" }
generalize = { prefix = "generalize/" }
release = { prefix = "release/" }  # optional, omit for cloud repos

[commands]
disable = []  # e.g. ["release"] for cloud repos

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

### Legacy Format

The original `[[branch_types]]` format is fully supported.

```toml
[[branch_types]]
name = "feature"
create = "feature/{NAME}"
from = "main"
to = [{ name = "main", strategy = "squash" }]
remote = "origin"

[[branch_types]]
name = "release"
create = "release/{NAME}"
from = "main"
to = [{ name = "main", strategy = "merge", tag = true }]
tag_pattern = "v{NAME}"
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
