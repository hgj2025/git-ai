#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# git-ai 开发者一键安装
#
# 用法（默认上报到 http://localhost:8080）：
#   curl -fsSL https://raw.githubusercontent.com/hgj2025/git-ai/main/dashboard/scripts/developer-setup.sh | bash
#
# 指定服务地址：
#   curl ... | bash -s -- --server http://your-server:8080
#
# 带 Token：
#   curl ... | bash -s -- --server http://your-server:8080 --token your-secret
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'
RED='\033[0;31m'; BOLD='\033[1m'; NC='\033[0m'

info()    { echo -e "${BLUE}[info]${NC} $*"; }
success() { echo -e "${GREEN}[ok]${NC} $*"; }
warn()    { echo -e "${YELLOW}[warn]${NC} $*"; }
error()   { echo -e "${RED}[error]${NC} $*" >&2; }
step()    { echo -e "\n${BOLD}▶ $*${NC}"; }

# ─── 解析参数 ──────────────────────────────────────────────────────────────
DEFAULT_SERVER="http://localhost:8080"
METRICS_SERVER="${GIT_AI_METRICS_SERVER:-$DEFAULT_SERVER}"
METRICS_TOKEN="${GIT_AI_METRICS_TOKEN:-}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --server)  METRICS_SERVER="$2"; shift 2 ;;
        --token)   METRICS_TOKEN="$2"; shift 2 ;;
        *)         shift ;;
    esac
done

echo ""
echo -e "${BOLD}git-ai 开发者一键安装${NC}"
echo -e "上报地址：${BLUE}${METRICS_SERVER}${NC}"
echo ""

# ─── 1. 检测 / 安装 git-ai CLI ─────────────────────────────────────────────
step "检查 git-ai"

if command -v git-ai &>/dev/null; then
    success "git-ai 已安装：$(git-ai --version 2>/dev/null || echo 'unknown')"
else
    info "git-ai 未安装，正在安装…"
    if [[ "$(uname)" == "Darwin" ]]; then
        if command -v brew &>/dev/null; then
            brew install git-ai-project/tap/git-ai
        else
            curl -fsSL https://usegitai.com/install.sh | bash
        fi
    elif [[ "$(uname)" == "Linux" ]]; then
        curl -fsSL https://usegitai.com/install.sh | bash
    else
        error "不支持的系统，请手动安装：https://usegitai.com/docs/installation"
        exit 1
    fi
    success "git-ai 安装完成：$(git-ai --version 2>/dev/null || echo '')"
fi

# ─── 2. 扫描 git 仓库 ──────────────────────────────────────────────────────
step "扫描本地 git 仓库"

SEARCH_DIRS=("$HOME/Code" "$HOME/Projects" "$HOME/workspace" "$HOME/src" "$HOME/dev")

FOUND_REPOS=()
for d in "${SEARCH_DIRS[@]}"; do
    [[ -d "$d" ]] || continue
    while IFS= read -r repo; do
        FOUND_REPOS+=("$repo")
    done < <(find "$d" -maxdepth 3 -name ".git" -type d 2>/dev/null | xargs -I{} dirname {})
done

if [[ ${#FOUND_REPOS[@]} -eq 0 ]]; then
    warn "在 ${SEARCH_DIRS[*]} 下未找到 git 仓库"
    warn "之后可在项目目录手动运行：git-ai install-hooks"
else
    info "找到 ${#FOUND_REPOS[@]} 个 git 仓库"
fi

# ─── 3. 安装 hooks ─────────────────────────────────────────────────────────
if [[ ${#FOUND_REPOS[@]} -gt 0 ]]; then
    step "安装 git hooks"
    HOOK_INSTALLED=0
    for repo in "${FOUND_REPOS[@]}"; do
        echo -n "  $repo … "
        if (cd "$repo" && git-ai install-hooks --quiet 2>/dev/null); then
            echo -e "${GREEN}ok${NC}"
            ((HOOK_INSTALLED++)) || true
        else
            echo -e "${YELLOW}跳过${NC}"
        fi
    done
    success "已在 $HOOK_INSTALLED 个仓库安装 hooks"
fi

# ─── 4. 配置 git notes 自动推送 ────────────────────────────────────────────
if [[ ${#FOUND_REPOS[@]} -gt 0 ]]; then
    step "配置 git notes 自动推送"
    NOTES_CONFIGURED=0
    for repo in "${FOUND_REPOS[@]}"; do
        (
            cd "$repo"
            remote=$(git remote 2>/dev/null | head -1)
            [[ -z "$remote" ]] && exit 0
            existing=$(git config --get-all remote."$remote".push 2>/dev/null | grep "notes/ai" || true)
            [[ -n "$existing" ]] && exit 0
            git config --add remote."$remote".push "refs/notes/ai:refs/notes/ai"
            git config --add remote."$remote".fetch "+refs/notes/ai:refs/notes/ai"
        ) 2>/dev/null && ((NOTES_CONFIGURED++)) || true
    done
    success "已在 $NOTES_CONFIGURED 个仓库配置 notes 推送"
fi

# ─── 5. 配置统计上报 ───────────────────────────────────────────────────────
step "配置统计上报"

git config --global git-ai.metrics-server "$METRICS_SERVER"
success "上报地址：$METRICS_SERVER"

if [[ -n "$METRICS_TOKEN" ]]; then
    git config --global git-ai.metrics-token "$METRICS_TOKEN"
    success "认证 Token 已配置"
fi

# 验证连通性
if curl -sf --max-time 3 "$METRICS_SERVER/api/stats" >/dev/null 2>&1; then
    success "服务连接正常"
else
    warn "无法连接 $METRICS_SERVER（服务可能未启动，不影响后续使用）"
fi

# ─── 完成 ──────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}✅ 安装完成！${NC}"
echo ""
echo "正常写代码、使用 AI 工具、git commit & push 即可。"
echo "AI 使用数据会自动记录，push 时自动上报到团队看板。"
echo ""
echo -e "  看板地址：${BLUE}${METRICS_SERVER}${NC}"
echo ""
echo "常用命令："
echo "  git-ai stats                    查看本地 AI 统计"
echo "  git-ai upload-metrics --verbose 手动上报（调试用）"
echo ""
