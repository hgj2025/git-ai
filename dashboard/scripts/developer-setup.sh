#!/usr/bin/env bash
# git-ai CLI developer setup
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/hgj2025/git-ai/main/dashboard/scripts/developer-setup.sh | bash
#   curl ... | bash -s -- --server http://your-server:8080

set -euo pipefail

# Ensure SHELL is set (may be unbound in curl | bash)
if [[ -z "${SHELL:-}" ]]; then
    if command -v zsh &>/dev/null; then
        SHELL="$(command -v zsh)"
    elif command -v bash &>/dev/null; then
        SHELL="$(command -v bash)"
    else
        SHELL="/bin/sh"
    fi
    export SHELL
fi

REPO="https://github.com/hgj2025/git-ai"
INSTALL_DIR="$HOME/.git-ai"
BIN="$INSTALL_DIR/bin/git-ai"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'
RED='\033[0;31m'; BOLD='\033[1m'; NC='\033[0m'

info()    { echo -e "${BLUE}[info]${NC} $*"; }
success() { echo -e "${GREEN}[ok]${NC} $*"; }
warn()    { echo -e "${YELLOW}[warn]${NC} $*"; }
error()   { echo -e "${RED}[error]${NC} $*" >&2; }
step()    { echo -e "\n${BOLD}> $*${NC}"; }

# --- parse args ---
SERVER="${GIT_AI_METRICS_SERVER:-http://localhost:8080}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --server) SERVER="$2"; shift 2 ;;
        *)        shift ;;
    esac
done

echo ""
echo -e "${BOLD}git-ai developer setup${NC}"
echo -e "metrics server: ${BLUE}${SERVER}${NC}"
echo ""

# --- 1. install CLI (skip if already installed) ---
step "check git-ai CLI"

if [[ -x "$BIN" ]]; then
    success "installed: $("$BIN" --version 2>/dev/null || echo 'unknown'), skip build"
elif command -v git-ai &>/dev/null; then
    success "installed: $(git-ai --version 2>/dev/null || echo 'unknown'), skip build"
else
    # ensure cargo is available
    [[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env" 2>/dev/null || true
    if ! command -v cargo &>/dev/null; then
        error "Rust toolchain required: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi

    info "cloning source..."
    rm -rf "$INSTALL_DIR/src"
    # use system git to avoid git-ai shim interference
    STD_GIT="$(command -v git)"
    for candidate in /usr/bin/git /opt/homebrew/bin/git /usr/local/bin/git; do
        if [[ -x "$candidate" ]] && [[ "$candidate" != *"git-ai"* ]]; then
            STD_GIT="$candidate"
            break
        fi
    done
    "$STD_GIT" clone --depth 1 "$REPO.git" "$INSTALL_DIR/src"

    info "building (first time may take 2-5 minutes)..."
    mkdir -p "$INSTALL_DIR/bin"
    if ! cargo build --release --manifest-path "$INSTALL_DIR/src/Cargo.toml"; then
        error "build failed"
        exit 1
    fi
    cp "$INSTALL_DIR/src/target/release/git-ai" "$BIN"

    # cleanup: keep only the binary
    rm -rf "$INSTALL_DIR/src"

    success "installed: $("$BIN" --version 2>/dev/null || echo '')"
fi

# ensure PATH includes git-ai (regardless of install path)
export PATH="$INSTALL_DIR/bin:$PATH"
SHELL_RC="$HOME/.zshrc"
if [[ "${SHELL:-}" == */bash ]]; then
    SHELL_RC="$HOME/.bashrc"
fi
if ! grep -qsF "$INSTALL_DIR/bin" "$SHELL_RC" 2>/dev/null; then
    echo "" >> "$SHELL_RC"
    echo "# Added by git-ai installer" >> "$SHELL_RC"
    echo "export PATH=\"$INSTALL_DIR/bin:\$PATH\"" >> "$SHELL_RC"
    info "added $INSTALL_DIR/bin to PATH ($SHELL_RC)"
else
    info "PATH already configured ($SHELL_RC)"
fi

# --- 2. install hooks for current repo ---
step "install git hooks"

if git rev-parse --git-dir &>/dev/null; then
    CURRENT_REPO=$(git rev-parse --show-toplevel)
    if "$BIN" install-hooks --quiet 2>/dev/null || git-ai install-hooks --quiet 2>/dev/null; then
        success "hooks installed: $CURRENT_REPO"
    else
        warn "hooks install failed, run manually: git-ai install-hooks"
    fi

    # configure git notes fetch (push is handled by git-ai post-push hook)
    remote=$(git remote 2>/dev/null | head -1)
    if [[ -n "$remote" ]]; then
        if ! git config --get-all remote."$remote".fetch 2>/dev/null | grep -q "notes/ai"; then
            git config --add remote."$remote".fetch "+refs/notes/ai:refs/notes/ai"
            success "git notes fetch configured"
        else
            info "git notes already configured"
        fi
    fi
else
    warn "not a git repo, skipping hooks"
    info "run in a project dir: git-ai install-hooks"
fi

# --- 3. configure metrics server (global, once) ---
step "configure metrics"

CURRENT_SERVER=$(git config --global git-ai.metrics-server 2>/dev/null || true)
if [[ "$CURRENT_SERVER" == "$SERVER" ]]; then
    info "metrics server already set: $SERVER"
else
    git config --global git-ai.metrics-server "$SERVER"
    success "metrics server: $SERVER"
fi

if curl -sf --max-time 3 "$SERVER/api/stats" >/dev/null 2>&1; then
    success "server reachable"
else
    warn "cannot reach $SERVER (non-blocking)"
fi

# --- done ---
echo ""
echo -e "${GREEN}${BOLD}done!${NC}"
echo ""
echo "commit & push as usual, AI metrics will be reported automatically."
echo ""
echo -e "  dashboard: ${BLUE}${SERVER}${NC}"
echo "  stats:     git-ai stats"
echo "  debug:     git-ai upload-metrics --verbose"
echo ""
