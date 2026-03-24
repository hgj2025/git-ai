#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# git-ai 一键卸载
#
# 用法：
#   curl -fsSL https://raw.githubusercontent.com/hgj2025/git-ai/main/dashboard/scripts/uninstall.sh | bash
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

INSTALL_DIR="$HOME/.git-ai"
BIN="$INSTALL_DIR/bin/git-ai"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'
RED='\033[0;31m'; BOLD='\033[1m'; NC='\033[0m'

info()    { echo -e "${BLUE}[info]${NC} $*"; }
success() { echo -e "${GREEN}[ok]${NC} $*"; }
warn()    { echo -e "${YELLOW}[warn]${NC} $*"; }
step()    { echo -e "\n${BOLD}▶ $*${NC}"; }

echo ""
echo -e "${BOLD}git-ai 一键卸载${NC}"
echo ""

# ─── 1. 卸载 hooks（当前仓库 + 编辑器集成）────────────────────────────────
step "卸载 hooks"

if [[ -x "$BIN" ]]; then
    if "$BIN" uninstall-hooks 2>/dev/null; then
        success "已卸载编辑器 hooks"
    else
        warn "编辑器 hooks 卸载失败（可能已清理）"
    fi
elif command -v git-ai &>/dev/null; then
    if git-ai uninstall-hooks 2>/dev/null; then
        success "已卸载编辑器 hooks"
    else
        warn "编辑器 hooks 卸载失败（可能已清理）"
    fi
else
    info "未找到 git-ai，跳过 hooks 卸载"
fi

# 清理当前仓库的 git notes 配置
if git rev-parse --git-dir &>/dev/null; then
    remote=$(git remote 2>/dev/null | head -1)
    if [[ -n "$remote" ]]; then
        git config --unset-all remote."$remote".push "refs/notes/ai" 2>/dev/null || true
        git config --unset-all remote."$remote".fetch "refs/notes/ai" 2>/dev/null || true
        success "已清理当前仓库 git notes 配置"
    fi
fi

# ─── 2. 清理全局 git 配置 ─────────────────────────────────────────────────
step "清理 git 配置"

removed_config=false
for key in git-ai.metrics-server git-ai.metrics-token; do
    if git config --global "$key" &>/dev/null; then
        git config --global --unset "$key"
        removed_config=true
    fi
done

if [[ "$removed_config" == true ]]; then
    success "已清理全局 git 配置"
else
    info "无全局配置需要清理"
fi

# ─── 3. 清理 shell 配置中的 PATH ──────────────────────────────────────────
step "清理 shell 配置"

cleaned_shell=false
for rc in "$HOME/.zshrc" "$HOME/.bashrc" "$HOME/.bash_profile" "$HOME/.config/fish/config.fish"; do
    if [[ -f "$rc" ]] && grep -qF "$INSTALL_DIR/bin" "$rc" 2>/dev/null; then
        # 删除包含 git-ai 安装目录的行和上方的注释行
        sed -i.bak "/.git-ai\/bin/d" "$rc"
        sed -i.bak "/Added by git-ai installer/d" "$rc"
        rm -f "${rc}.bak"
        cleaned_shell=true
        success "已清理 $rc"
    fi
done

if [[ "$cleaned_shell" == false ]]; then
    info "shell 配置无需清理"
fi

# ─── 4. 删除 symlink ──────────────────────────────────────────────────────
if [[ -L "$HOME/.local/bin/git-ai" ]]; then
    rm -f "$HOME/.local/bin/git-ai"
    success "已删除 ~/.local/bin/git-ai"
fi

# ─── 5. 删除安装目录 ──────────────────────────────────────────────────────
step "删除 git-ai"

if [[ -d "$INSTALL_DIR" ]]; then
    rm -rf "$INSTALL_DIR"
    success "已删除 $INSTALL_DIR"
else
    info "$INSTALL_DIR 不存在，跳过"
fi

# ─── 完成 ──────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}卸载完成！${NC}"
echo ""
echo "请重新打开终端使 PATH 变更生效。"
echo ""
