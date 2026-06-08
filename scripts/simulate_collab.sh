#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Git flow Multi-User Collaboration Simulation ===${NC}"

# Find absolute path of the workspace
WORKSPACE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GITFLOW_BIN="$WORKSPACE_DIR/target/debug/git-flow"

if [ ! -f "$GITFLOW_BIN" ]; then
    echo -e "${YELLOW}Building git-flow binary...${NC}"
    cargo build
fi

# Create temp directory for collaboration simulation
TEMP_DIR=$(mktemp -d -t gitflow-collab-XXXXXX)
echo -e "Simulation directory: ${GREEN}$TEMP_DIR${NC}"

# Cleanup on exit
cleanup() {
    rm -rf "$TEMP_DIR"
    echo -e "${BLUE}Cleaned up simulation directory.${NC}"
}
trap cleanup EXIT

# 1. Setup Bare Remote Repository
echo -e "\n${BLUE}1. Setting up bare remote repository...${NC}"
mkdir -p "$TEMP_DIR/remote.git"
git init --bare "$TEMP_DIR/remote.git"

# 2. Setup Developer A
echo -e "\n${BLUE}2. Setting up Developer A clone...${NC}"
git clone "$TEMP_DIR/remote.git" "$TEMP_DIR/dev_a"
cd "$TEMP_DIR/dev_a"
git config user.name "Developer A"
git config user.email "deva@example.com"

# Setup initial branches
echo "Initial code" > a.txt
git add a.txt
git commit -m "Initial commit"
git branch -M main
git push origin main

git checkout -b dev
git push origin dev

# Setup .gitflow.toml
cat << 'EOF' > .gitflow.toml
allow_non_main_base = true

[[branch_types]]
name = "feature"
create = "feature/{NAME}"
from = "dev"
to = [{ name = "dev", strategy = "merge" }]
remote = "origin"
EOF

git add .gitflow.toml
git commit -m "Configure gitflow"
git push origin dev

# 3. Setup Developer B
echo -e "\n${BLUE}3. Setting up Developer B clone...${NC}"
git clone "$TEMP_DIR/remote.git" "$TEMP_DIR/dev_b"
cd "$TEMP_DIR/dev_b"
git config user.name "Developer B"
git config user.email "devb@example.com"
git checkout dev

# 4. Developer A starts a feature branch
echo -e "\n${BLUE}4. Developer A starts a new feature branch...${NC}"
cd "$TEMP_DIR/dev_a"
"$GITFLOW_BIN" feature start feat1

# Developer A adds code and commits
echo "Code by A" >> a.txt
git add a.txt
git commit -m "Dev A work"

# Developer A publishes the feature branch to remote
echo -e "\n${BLUE}5. Developer A publishes the feature branch...${NC}"
"$GITFLOW_BIN" feature publish

# 5. Developer B tracks the feature branch
echo -e "\n${BLUE}6. Developer B tracks the feature branch...${NC}"
cd "$TEMP_DIR/dev_b"
# Fetch remote branches first
git fetch origin
git checkout feature/feat1
"$GITFLOW_BIN" feature track feat1

# Developer B commits more work
echo "Code by B" >> a.txt
git add a.txt
git commit -m "Dev B work"

# Developer B publishes (pushes) B's changes to the published branch
echo -e "\n${BLUE}7. Developer B publishes changes on the feature...${NC}"
"$GITFLOW_BIN" feature publish

# 6. Developer A pulls Developer B's changes
echo -e "\n${BLUE}8. Developer A updates their feature branch...${NC}"
cd "$TEMP_DIR/dev_a"
git pull origin feature/feat1

# 7. Developer A finishes the feature branch
echo -e "\n${BLUE}9. Developer A finishes and pushes the feature branch...${NC}"
# Finish with --push flag to push dev to remote and delete remote feature branch
"$GITFLOW_BIN" feature finish feat1 --push

# 8. Developer B updates dev and deletes the local branch
echo -e "\n${BLUE}10. Developer B updates dev and cleans up local branch...${NC}"
cd "$TEMP_DIR/dev_b"
git checkout dev
git pull origin dev

# Now delete B's local branch since it's fully merged
"$GITFLOW_BIN" feature delete feat1

echo -e "\n${GREEN}=== Collaboration Simulation Succeeded! ===${NC}"
