#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# git-ai CLI 开发者安装
#
# 首次安装（编译 + hooks + 配置）：
#   curl -fsSL https://raw.githubusercontent.com/hgj2025/git-ai/main/dashboard/scripts/developer-setup.sh | bash
#
# 在新仓库中执行（跳过编译，只装 hooks）：
#   curl -fsSL ... | bash
#
# 指定服务地址：
#   curl ... | bash -s -- --server http://your-server:8080
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

REPO="https://github.com/hgj2025/git-ai"
INSTALL_DIR="$HOME/.git-ai"
BIN="$INSTALL_DIR/bin/git-ai"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'
RED='\033[0;31m'; BOLD='\033[1m'; NC='\033[0m'

info()    { echo -e "${BLUE}[info]${NC} $*"; }
success() { echo -e "${GREEN}[ok]${NC} $*"; }
warn()    { echo -e "${YELLOW}[warn]${NC} $*"; }
error()   { echo -e "${RED}[error]${NC} $*" >&2; }
step()    { echo -e "\n${BOLD}▶ $*${NC}"; }

# ─── 解析参数 ──────────────────────────────────────────────────────────────
SERVER="${GIT_AI_METRICS_SERVER:-http://localhost:8080}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --server) SERVER="$2"; shift 2 ;;
        *)        shift ;;
    esac
done

echo ""
echo -e "${BOLD}git-ai 开发者安装${NC}"
echo -e "上报地址：${BLUE}${SERVER}${NC}"
echo ""

# ─── 1. 安装 CLI（已安装则跳过）─────────────────────────────────────────
step "检查 git-ai CLI"

# 优先检查已安装的二进制，不依赖 PATH
if [[ -x "$BIN" ]]; then
    success "已安装：$("$BIN" --version 2>/dev/null || echo 'unknown')，跳过编译"
elif command -v git-ai &>/dev/null; then
    success "已安装：$(git-ai --version 2>/dev/null || echo 'unknown')，跳过编译"
else
    # 确保 cargo 可用
    [[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env" 2>/dev/null || true
    if ! command -v cargo &>/dev/null; then
        error "需要 Rust 工具链：curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi

    info "克隆源码…"
    rm -rf "$INSTALL_DIR/src"
    # 使用系统 git 避免 git-ai shim 干扰
    STD_GIT=$(command -v git)
    for candidate in /usr/bin/git /opt/homebrew/bin/git /usr/local/bin/git; do
        if [[ -x "$candidate" ]] && [[ "$candidate" != *"git-ai"* ]]; then
            STD_GIT="$candidate"
            break
        fi
    done
    "$STD_GIT" clone --depth 1 "$REPO.git" "$INSTALL_DIR/src"

    info "编译中（首次约 2-5 分钟，请耐心等待）…"
    mkdir -p "$INSTALL_DIR/bin"
    if ! cargo build --release --manifest-path "$INSTALL_DIR/src/Cargo.toml"; then
        error "编译失败，请检查上方错误信息"
        exit 1
    fi
    cp "$INSTALL_DIR/src/target/release/git-ai" "$BIN"

    # 清理：只保留二进制
    rm -rf "$INSTALL_DIR/src"

    success "安装完成：$("$BIN" --version 2>/dev/null || echo '')"
fi

# 确保 PATH 中包含 git-ai（不管是新装还是已装）
export PATH="$INSTALL_DIR/bin:$PATH"
SHELL_RC="$HOME/.zshrc"
[[ "${SHELL:-}" == */bash ]] && SHELL_RC="$HOME/.bashrc"
if ! grep -qsF "$INSTALL_DIR/bin" "$SHELL_RC" 2>/dev/null; then
    echo "" >> "$SHELL_RC"
    echo "# Added by git-ai installer" >> "$SHELL_RC"
    echo "export PATH=\"$INSTALL_DIR/bin:\$PATH\"" >> "$SHELL_RC"
    info "已添加 $INSTALL_DIR/bin 到 PATH（$SHELL_RC）"
else
    info "PATH 已配置（$SHELL_RC）"
fi

# ─── 2. 当前仓库安装 hooks + notes 推送 ──────────────────────────────────
step "安装 git hooks"

if git rev-parse --git-dir &>/dev/null; then
    CURRENT_REPO=$(git rev-parse --show-toplevel)
    if "$BIN" install-hooks --quiet 2>/dev/null || git-ai install-hooks --quiet 2>/dev/null; then
        success "已安装 hooks：$CURRENT_REPO"
    else
        warn "hooks 安装失败，之后可手动运行：git-ai install-hooks"
    fi

    # 配置 git notes 推送
    remote=$(git remote 2>/dev/null | head -1)
    if [[ -n "$remote" ]]; then
        if ! git config --get-all remote."$remote".push 2>/dev/null | grep -q "notes/ai"; then
            git config --add remote."$remote".push "refs/notes/ai:refs/notes/ai"
            git config --add remote."$remote".fetch "+refs/notes/ai:refs/notes/ai"
            success "已配置 git notes 推送"
        else
            info "git notes 推送已配置"
        fi
    fi
else
    warn "当前目录不是 git 仓库，跳过 hooks 安装"
    info "请在项目目录运行：git-ai install-hooks"
fi

# ─── 3. 配置上报（全局，只需配一次）──────────────────────────────────────
step "配置统计上报"

CURRENT_SERVER=$(git config --global git-ai.metrics-server 2>/dev/null || true)
if [[ "$CURRENT_SERVER" == "$SERVER" ]]; then
    info "上报地址已配置：$SERVER"
else
    git config --global git-ai.metrics-server "$SERVER"
    success "上报地址：$SERVER"
fi

if curl -sf --max-time 3 "$SERVER/api/stats" >/dev/null 2>&1; then
    success "服务连接正常"
else
    warn "无法连接 $SERVER（不影响后续使用）"
fi

# ─── 完成 ──────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}✅ 安装完成！${NC}"
echo ""
echo "正常 commit & push 即可，AI 数据会自动上报。"
echo ""
echo -e "  看板：${BLUE}${SERVER}${NC}"
echo "  统计：git-ai stats"
echo "  调试：git-ai upload-metrics --verbose"
echo ""
