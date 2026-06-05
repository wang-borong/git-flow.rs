#!/usr/bin/env bash
# simulate_hotfix_multitarget.sh
# Simulates: A critical hotfix that must simultaneously merge to main AND dev.
# Tests: multi-target finish, workspace restore, branch cleanup, release flow.
set -uo pipefail

# Resolve GITFLOW to absolute path BEFORE cd to TMPDIR
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
GITFLOW="${GITFLOW:-$PROJECT_ROOT/target/debug/gitflow}"

# Ensure binary exists
if [ ! -x "$GITFLOW" ]; then
    echo "Building gitflow binary..."
    (cd "$PROJECT_ROOT" && cargo build -q 2>&1)
    GITFLOW="$PROJECT_ROOT/target/debug/gitflow"
fi

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
PASS=0; FAIL=0

pass() { echo -e "  ${GREEN}✔${NC} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "  ${RED}✗${NC} $1"; FAIL=$((FAIL + 1)); }
section() { echo -e "\n${CYAN}▶ $1${NC}"; }

gf() { "$GITFLOW" "$@" 2>&1; }

# ---- Setup ----
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

cd "$TMPDIR"
git init -b main . > /dev/null
git config user.name "testuser"
git config user.email "test@test.com"

echo "init" > README.md
git add . && git commit -m "initial commit" > /dev/null

git checkout -b dev > /dev/null
cat > .gitflow.toml << 'EOF'
allow_non_main_base = true

[[branch_types]]
name = "feature"
create = "feature/{NAME}"
from = "dev"
to = [{ name = "dev", strategy = "merge" }]

[[branch_types]]
name = "hotfix"
create = "hotfix/{NAME}"
from = "main"
to = [
  { name = "main", strategy = "merge" },
  { name = "dev", strategy = "merge" }
]

[[branch_types]]
name = "release"
create = "release/{NAME}"
from = "dev"
to = [
  { name = "main", strategy = "merge" },
  { name = "dev", strategy = "merge" }
]
EOF

git add .gitflow.toml && git commit -m "add gitflow config" > /dev/null
git checkout main > /dev/null && git merge dev --no-edit > /dev/null && git checkout dev > /dev/null

echo -e "${YELLOW}=== Hotfix Multi-Target Simulation ===${NC}"
echo "Scenario: Critical production bug found; must be fixed on main and propagated to dev"

# ============================================================
section "Step 1: Developer detects production bug, starts hotfix"
git checkout main > /dev/null 2>&1
gf hotfix start critical-null-ptr
CURRENT=$(git rev-parse --abbrev-ref HEAD)
if [ "$CURRENT" == "hotfix/critical-null-ptr" ]; then
    pass "Switched to hotfix/critical-null-ptr"
else
    fail "Expected hotfix/critical-null-ptr, got $CURRENT"
fi

# ============================================================
section "Step 2: Fix the bug"
echo "fix: prevent null pointer dereference" >> bugfix.c
git add bugfix.c > /dev/null
git commit -m "fix: critical null pointer" > /dev/null
pass "Committed hotfix on hotfix/critical-null-ptr"

# ============================================================
section "Step 3: Finish hotfix (merges to main AND dev)"
gf hotfix finish critical-null-ptr
CURRENT=$(git rev-parse --abbrev-ref HEAD)
if [ "$CURRENT" == "main" ] || [ "$CURRENT" == "dev" ]; then
    pass "After finish, on target branch: $CURRENT"
else
    fail "Expected main or dev after finish, got $CURRENT"
fi

# Branch should be gone
if ! git branch --list hotfix/critical-null-ptr | grep -q .; then
    pass "hotfix/critical-null-ptr branch deleted"
else
    fail "hotfix/critical-null-ptr still exists"
fi

# ============================================================
section "Step 4: Verify fix is on MAIN"
git checkout main > /dev/null 2>&1
if [ -f bugfix.c ]; then
    pass "bugfix.c exists on main"
else
    fail "bugfix.c MISSING from main"
fi

# ============================================================
section "Step 5: Verify fix is on DEV (multi-target propagation)"
git checkout dev > /dev/null 2>&1
if [ -f bugfix.c ]; then
    pass "bugfix.c exists on dev (correctly propagated)"
else
    fail "bugfix.c MISSING from dev (multi-target merge failed!)"
fi

# ============================================================
section "Step 6: Simulate release flow (dev → main + dev)"
git checkout dev > /dev/null 2>&1
echo "v2.0 release prep" > CHANGELOG.md
git add CHANGELOG.md && git commit -m "prepare v2.0" > /dev/null

gf release start v2.0.0
CURRENT=$(git rev-parse --abbrev-ref HEAD)
if [ "$CURRENT" == "release/v2.0.0" ]; then
    pass "On release/v2.0.0"
else
    fail "Expected release/v2.0.0, got $CURRENT"
fi

echo "Final release notes" >> CHANGELOG.md
git add CHANGELOG.md && git commit -m "finalize release notes" > /dev/null

gf release finish v2.0.0

# Release branch should be gone
if ! git branch --list release/v2.0.0 | grep -q .; then
    pass "release/v2.0.0 branch deleted"
else
    fail "release/v2.0.0 still exists"
fi

git checkout main > /dev/null 2>&1
if [ -f CHANGELOG.md ]; then
    pass "CHANGELOG.md on main"
else
    fail "CHANGELOG.md missing from main"
fi

git checkout dev > /dev/null 2>&1
if [ -f CHANGELOG.md ]; then
    pass "CHANGELOG.md on dev"
else
    fail "CHANGELOG.md missing from dev"
fi

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
