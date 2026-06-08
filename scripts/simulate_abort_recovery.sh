#!/usr/bin/env bash
# simulate_abort_recovery.sh
# Simulates: finish/sync conflict → abort → workspace fully restored.
# Tests: GITFLOW_STATE saved, abort clears state, original branch restored.
set -uo pipefail

# Resolve GITFLOW to absolute path BEFORE cd to TMPDIR
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
GITFLOW="${GITFLOW:-$PROJECT_ROOT/target/debug/git-flow}"

# Ensure binary exists
if [ ! -x "$GITFLOW" ]; then
    echo "Building git-flow binary..."
    (cd "$PROJECT_ROOT" && cargo build -q 2>&1)
fi

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
PASS=0; FAIL=0

pass() { echo -e "  ${GREEN}✔${NC} $1"; ((PASS++)); }
fail() { echo -e "  ${RED}✗${NC} $1"; ((FAIL++)); }
section() { echo -e "\n${CYAN}▶ $1${NC}"; }
check_branch() { git -C "$1" rev-parse --abbrev-ref HEAD; }
state_exists() { [ -f ".git/GITFLOW_STATE" ]; }

# ---- Setup ----
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

cd "$TMPDIR"
git init -b main .
git config user.name "testuser"
git config user.email "test@test.com"
git config merge.conflictstyle "merge"

echo "base" > conflict.txt
git add . && git commit -m "initial"

git checkout -b dev
cat > .gitflow.toml << 'EOF'
allow_non_main_base = true

[[branch_types]]
name = "feature"
create = "feature/{NAME}"
from = "dev"
to = [{ name = "dev", strategy = "merge" }]

[[branch_types]]
name = "customer"
create = "customer/{NAME}"
from = "dev"
to = [{ name = "dev", strategy = "merge" }]
EOF
git add .gitflow.toml && git commit -m "add config"
git checkout main && git merge dev --no-edit && git checkout dev

echo -e "${YELLOW}=== Abort Recovery Simulation ===${NC}"

# ============================================================
section "Scenario A: feature finish → conflict → abort → verify workspace restored"

"$GITFLOW" feature start conflict-feature
echo "feature change" > conflict.txt
git add conflict.txt && git commit -m "feature changes conflict.txt"

# Create conflict on dev
git checkout dev
echo "dev change" > conflict.txt
git add conflict.txt && git commit -m "dev changes conflict.txt (creates conflict)"

git checkout feature/conflict-feature

# Try finish — expect conflict
echo "--- trying to finish (expecting conflict) ---"
"$GITFLOW" feature finish conflict-feature || true

# GITFLOW_STATE should exist after conflict
if state_exists; then
    pass "GITFLOW_STATE saved after conflict"
else
    fail "GITFLOW_STATE NOT saved after conflict"
fi

# Abort the merge
git merge --abort 2>/dev/null || git rebase --abort 2>/dev/null || true

# Run git flow abort
"$GITFLOW" abort
CURRENT=$(check_branch .)

# State should be cleared
if ! state_exists; then
    pass "GITFLOW_STATE cleared after abort"
else
    fail "GITFLOW_STATE still exists after abort"
fi

# Should be restored to feature branch (ISSUE-A1 fix)
if [ "$CURRENT" == "feature/conflict-feature" ]; then
    pass "Restored to feature/conflict-feature after abort (ISSUE-A1 fix)"
else
    fail "Expected feature/conflict-feature, got $CURRENT (ISSUE-A1 regression)"
fi

# Feature branch should still exist (abort doesn't delete it)
if git branch --list "feature/conflict-feature" | grep -q .; then
    pass "feature/conflict-feature still exists (not accidentally deleted)"
else
    fail "feature/conflict-feature was incorrectly deleted by abort"
fi

# ============================================================
section "Scenario B: custom sync conflict → state saved → abort clears state"

git checkout dev > /dev/null 2>&1
"$GITFLOW" custom start acme-corp 2>&1

# First, make a conflicting change on customer branch
git checkout customer/acme-corp > /dev/null 2>&1
echo "customer version" > sync_conflict.txt
git add sync_conflict.txt && git commit -m "customer: sync_conflict.txt"

# Now make a CONFLICTING change on main (which sync pulls from)
git checkout main > /dev/null 2>&1
echo "main version - will conflict" > sync_conflict.txt
git add sync_conflict.txt && git commit -m "main: conflicting sync_conflict.txt"

# Switch back to dev (the "current" branch before sync)
git checkout dev > /dev/null 2>&1

# Try sync acme-corp — should trigger conflict (customer/acme-corp vs main)
echo "--- trying custom sync (expecting merge conflict) ---"
"$GITFLOW" custom sync acme-corp 2>&1 || true

if state_exists; then
    pass "GITFLOW_STATE::Sync saved after sync conflict"
    echo "    State key: $(python3 -c 'import sys,json; f=open(".git/GITFLOW_STATE"); d=json.load(f); print(list(d.keys())[0])' 2>/dev/null || head -1 .git/GITFLOW_STATE)"
else
    fail "GITFLOW_STATE NOT saved after sync conflict (check sync_customer conflict detection)"
fi

# Abort everything
git checkout customer/acme-corp > /dev/null 2>&1
git merge --abort 2>/dev/null || true
"$GITFLOW" abort 2>&1 || true

if ! state_exists; then
    pass "GITFLOW_STATE cleared after abort"
else
    fail "GITFLOW_STATE still exists after abort"
fi

# ============================================================
section "Scenario C: Verify no GITFLOW_STATE left after clean finish"

git checkout dev
"$GITFLOW" feature start clean-finish
echo "clean work" > clean.txt
git add clean.txt && git commit -m "clean work"
"$GITFLOW" feature finish clean-finish

if ! state_exists; then
    pass "No GITFLOW_STATE after clean finish"
else
    fail "GITFLOW_STATE unexpectedly present after clean finish"
fi

CURRENT=$(check_branch .)
[ "$CURRENT" == "dev" ] && pass "Correctly on dev after clean finish" || fail "Expected dev, got $CURRENT"

# ============================================================
echo ""
echo -e "${YELLOW}=== Results ===${NC}"
echo -e "  ${GREEN}Passed:${NC} $PASS"
echo -e "  ${RED}Failed:${NC} $FAIL"

if [ "$FAIL" -gt 0 ]; then
    echo -e "\n${RED}SIMULATION FAILED${NC}"
    exit 1
else
    echo -e "\n${GREEN}SIMULATION PASSED${NC}"
fi
