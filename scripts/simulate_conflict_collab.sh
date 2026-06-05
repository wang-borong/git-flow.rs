#!/bin/bash
set -e

# ==============================================================================
# Gitflow Conflict Resolution Simulation Script
# ==============================================================================
# This script simulates a scenario where two developers work concurrently and
# encounter a merge conflict when finishing a feature branch.
# It tests the `"$GITFLOW_BIN" continue` functionality.
# ==============================================================================

# Compile the latest binary first
cargo build
GITFLOW_BIN="$(pwd)/target/debug/gitflow"
if [ ! -f "$GITFLOW_BIN" ]; then
    echo "[-] ERROR: "$GITFLOW_BIN" binary not found. Run cargo build."
    exit 1
fi

# Create a temporary directory for the simulation
SIM_DIR=$(mktemp -d)
echo "Setting up simulation in $SIM_DIR"

# Cleanup function
cleanup() {
    echo "Cleaning up $SIM_DIR..."
    rm -rf "$SIM_DIR"
}
trap cleanup EXIT

# 1. Setup Remote Repository
cd "$SIM_DIR"
mkdir remote_repo.git
cd remote_repo.git
git init --bare > /dev/null
echo "[+] Remote repository initialized."

# 2. Setup Developer A
cd "$SIM_DIR"
git clone remote_repo.git dev_a > /dev/null 2>&1
cd dev_a

# Configure Developer A
git config user.name "Developer A"
git config user.email "deva@example.com"

# Create initial commit and config
echo "Initial content" > file.txt
git add file.txt
git commit -m "Initial commit" > /dev/null
git push origin main > /dev/null 2>&1

cat > .gitflow.toml << 'EOF'
allow_non_main_base = true

[[branch_types]]
name = "feature"
create = "feature/{NAME}"
from = "main"
to = [{ name = "main", strategy = "merge" }]
EOF
git add .gitflow.toml
git commit -m "Add "$GITFLOW_BIN" config" > /dev/null
git push origin main > /dev/null 2>&1
echo "[+] Developer A pushed initial config."

# 3. Setup Developer B
cd "$SIM_DIR"
git clone remote_repo.git dev_b > /dev/null 2>&1
cd dev_b

# Configure Developer B
git config user.name "Developer B"
git config user.email "devb@example.com"
echo "[+] Developer B cloned repository."

# 4. Developer B starts a feature branch
echo "[+] Developer B starting feature branch..."
"$GITFLOW_BIN" feature start my-feature > /dev/null
echo "Developer B changes" > file.txt
git commit -am "Dev B feat commit" > /dev/null

# 5. Meanwhile, Developer A pushes a conflicting change to main
cd "$SIM_DIR/dev_a"
echo "Developer A conflicting changes" > file.txt
git commit -am "Dev A conflicting commit" > /dev/null
git push origin main > /dev/null 2>&1
echo "[+] Developer A pushed conflicting changes to main."

# 6. Developer B finishes the feature branch
cd "$SIM_DIR/dev_b"
# Developer B updates their local main to get Developer A's changes
git checkout main > /dev/null 2>&1
git pull origin main > /dev/null 2>&1
git checkout feature/my-feature > /dev/null 2>&1

echo "[+] Developer B attempting to finish feature branch (should conflict)..."
# We expect this to fail due to conflict, so we don't use set -e for this command
set +e
# Use non-interactive yes (though it should fail before prompt if not pushing)
# Note: Since there's no prompt required unless pushing/deleting in interactive mode
# we will just run the command and pipe yes to handle branch deletion prompts later
yes | "$GITFLOW_BIN" feature finish my-feature > finish_output.txt 2>&1
set -e

echo "[+] "$GITFLOW_BIN" finish encountered a merge conflict."
if ! grep -q "conflict" finish_output.txt; then
    echo "[-] ERROR: finish output didn't mention conflict. Output was:"
    cat finish_output.txt
    exit 1
fi

if ! [ -f ".git/GITFLOW_STATE" ]; then
    echo "[-] ERROR: GITFLOW_STATE file was not created!"
    cat finish_output.txt
    exit 1
fi
echo "[+] GITFLOW_STATE file successfully created."

# 7. Developer B resolves conflict and continues
echo "[+] Developer B resolving conflicts..."
# We keep Dev B's changes
echo "Developer B changes" > file.txt
git add file.txt
# No need to run git commit, `"$GITFLOW_BIN" continue` will do it or resume the merge!
# Wait, merge conflict resolution requires `git commit` usually? 
# Oh, "$GITFLOW_BIN" continue runs `git merge --continue` which commits.
echo "[+] Developer B running "$GITFLOW_BIN" continue..."
# Set GIT_EDITOR=true to avoid opening vi for the merge commit message
yes | GIT_EDITOR=true "$GITFLOW_BIN" continue > continue_output.txt 2>&1

echo "[+] "$GITFLOW_BIN" continue completed."

if [ -f ".git/GITFLOW_STATE" ]; then
    echo "[-] ERROR: GITFLOW_STATE file was not cleaned up!"
    exit 1
fi
echo "[+] GITFLOW_STATE file was properly cleaned up."

# Verify that the branch is merged and we are on main
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$CURRENT_BRANCH" != "main" ]; then
    echo "[-] ERROR: Developer B is on $CURRENT_BRANCH instead of main."
    exit 1
fi

# Verify the feature branch is deleted
if git branch | grep -q "feature/my-feature"; then
    echo "[-] ERROR: feature branch was not deleted."
    exit 1
fi

# Verify the file content
FILE_CONTENT=$(cat file.txt)
if [ "$FILE_CONTENT" != "Developer B changes" ]; then
    echo "[-] ERROR: Unexpected file content."
    exit 1
fi

echo "=============================================================================="
echo "[+] SUCCESS: Conflict simulation passed!"
echo "=============================================================================="
