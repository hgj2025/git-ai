#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# git-ai 开发者一键配置脚本
#
# 用法：
#   curl -fsSL https://your-team-server/developer-setup.sh | bash
#   或本地执行：bash scripts/developer-setup.sh
#
# 支持环境变量预设（适合内网分发）：
#   GIT_AI_METRICS_SERVER=http://team-server:8080 \
#   GIT_AI_METRICS_TOKEN=your-token \
#   bash developer-setup.sh
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'
BOLD='\033[1m'; NC='\033[0m'

info()    { echo -e "${BLUE}[info]${NC} $*"; }
success() { echo -e "${GREEN}[ok]${NC} $*"; }
warn()    { echo -e "${YELLOW}[warn]${NC} $*"; }
error()   { echo -e "${RED}[error]${NC} $*" >&2; }
step()    { echo -e "\n${BOLD}▶ $*${NC}"; }

# ─── 0. 读取团队统计服务配置 ─────────────────────────────────────────────────
# 默认指向本地服务，正式部署时替换为团队域名
DEFAULT_METRICS_SERVER="http://localhost:8080"

METRICS_SERVER="${GIT_AI_METRICS_SERVER:-$DEFAULT_METRICS_SERVER}"
METRICS_TOKEN="${GIT_AI_METRICS_TOKEN:-}"

if [ -t 0 ]; then
    # 交互模式下允许修改
    echo ""
    echo -e "${BOLD}团队 AI 统计服务配置${NC}"
    echo "正式部署后请替换为团队服务地址（如 http://your-domain:8080）"
    echo ""
    read -rp "统计服务地址 [${METRICS_SERVER}]: " input_server || true
    if [ -n "$input_server" ]; then
        METRICS_SERVER="$input_server"
    fi
    if [ -z "$METRICS_TOKEN" ]; then
        read -rsp "认证 Token (无则留空): " METRICS_TOKEN || true
        echo ""
    fi
fi

# ─── 1. 检测 git-ai 是否安装 ──────────────────────────────────────────────────
step "检查 git-ai 安装状态"

if command -v git-ai &>/dev/null; then
    GITAI_VER=$(git-ai --version 2>/dev/null || echo "unknown")
    success "git-ai 已安装：$GITAI_VER"
else
    info "git-ai 未安装，开始安装…"
    if [[ "$(uname)" == "Darwin" ]]; then
        if command -v brew &>/dev/null; then
            brew install git-ai-project/tap/git-ai
        else
            curl -fsSL https://usegitai.com/install.sh | bash
        fi
    elif [[ "$(uname)" == "Linux" ]]; then
        curl -fsSL https://usegitai.com/install.sh | bash
    else
        error "不支持的系统，请手动安装 git-ai：https://usegitai.com/docs/installation"
        exit 1
    fi
    success "git-ai 安装完成"
fi

# ─── 2. 在所有 git 仓库中安装 hooks ──────────────────────────────────────────
step "安装 git hooks"

# 找出当前用户所有常用代码仓库（~/{Code,Projects,workspace,src} 下的 git 仓库）
SEARCH_DIRS=(
    "$HOME/Code"
    "$HOME/Projects"
    "$HOME/workspace"
    "$HOME/src"
    "$HOME/dev"
)

FOUND_REPOS=()
for d in "${SEARCH_DIRS[@]}"; do
    if [[ -d "$d" ]]; then
        while IFS= read -r repo; do
            FOUND_REPOS+=("$repo")
        done < <(find "$d" -maxdepth 3 -name ".git" -type d 2>/dev/null | xargs -I{} dirname {})
    fi
done

if [[ ${#FOUND_REPOS[@]} -eq 0 ]]; then
    warn "未找到 git 仓库，跳过 hooks 安装"
    warn "请手动在每个仓库中运行：git-ai install-hooks"
else
    info "找到 ${#FOUND_REPOS[@]} 个 git 仓库"
    for repo in "${FOUND_REPOS[@]}"; do
        echo -n "  安装 hooks: $repo … "
        if (cd "$repo" && git-ai install-hooks --quiet 2>/dev/null); then
            echo -e "${GREEN}ok${NC}"
        else
            echo -e "${YELLOW}跳过${NC}"
        fi
    done
fi

# ─── 3. 配置 git notes 自动推送 ───────────────────────────────────────────────
step "配置 git notes 自动推送"

cat << 'EOF'
为了让看板能收集你的 AI 数据，需要在每个仓库配置 git notes 推送。
EOF

configure_notes_push() {
    local repo="$1"
    cd "$repo"

    # 检查是否有 remote
    local remote
    remote=$(git remote 2>/dev/null | head -1)
    if [[ -z "$remote" ]]; then
        return 0
    fi

    # 检查是否已配置
    local existing
    existing=$(git config --get-all remote."$remote".push 2>/dev/null | grep "notes/ai" || true)
    if [[ -n "$existing" ]]; then
        return 0
    fi

    git config --add remote."$remote".push "refs/notes/ai:refs/notes/ai"
    git config --add remote."$remote".fetch "+refs/notes/ai:refs/notes/ai"
}

CONFIGURED=0
for repo in "${FOUND_REPOS[@]}"; do
    if configure_notes_push "$repo" 2>/dev/null; then
        ((CONFIGURED++)) || true
    fi
done

success "已在 $CONFIGURED 个仓库配置 git notes 推送"

# ─── 4. 配置团队统计服务 ─────────────────────────────────────────────────────
step "配置团队统计服务"

if [ -n "$METRICS_SERVER" ]; then
    git config --global git-ai.metrics-server "$METRICS_SERVER"
    success "已设置 git-ai.metrics-server = $METRICS_SERVER"

    if [ -n "$METRICS_TOKEN" ]; then
        git config --global git-ai.metrics-token "$METRICS_TOKEN"
        success "已设置 git-ai.metrics-token"
    fi

    # 验证连通性
    if command -v curl &>/dev/null; then
        if curl -sf --max-time 5 "$METRICS_SERVER/api/stats" >/dev/null 2>&1; then
            success "连接统计服务成功 ✓"
        else
            warn "无法连接 $METRICS_SERVER，请确认服务已启动或稍后手动验证"
        fi
    fi
else
    info "未配置统计服务，跳过"
    info "之后可运行：git config --global git-ai.metrics-server <url>"
fi

# ─── 5. 全局 git 配置（可选） ─────────────────────────────────────────────────
step "全局 git 配置"

# 对新克隆的仓库自动 fetch notes
if ! git config --global --get-all remote.origin.fetch 2>/dev/null | grep -q "notes/ai"; then
    # 注意：全局 fetch refspec 对部分 git 版本有兼容性问题，仅作提示
    info "提示：新克隆的仓库需手动运行以下命令配置 notes 推送："
    echo ""
    echo "    git config --add remote.origin.push 'refs/notes/ai:refs/notes/ai'"
    echo "    git config --add remote.origin.fetch '+refs/notes/ai:refs/notes/ai'"
    echo ""
    info "或将以下内容加入你的 ~/.gitconfig [alias] 段，方便一键配置："
    echo ""
    echo "    [alias]"
    echo "        ai-setup = !git config --add remote.origin.push 'refs/notes/ai:refs/notes/ai' && git config --add remote.origin.fetch '+refs/notes/ai:refs/notes/ai'"
    echo ""
fi

# ─── 6. 验证 ──────────────────────────────────────────────────────────────────
step "验证安装"

echo ""
echo "  git-ai 版本：$(git-ai --version 2>/dev/null || echo '未检测到')"
echo ""

# 检查 hooks 是否生效
HOOK_OK=0
for repo in "${FOUND_REPOS[@]:0:3}"; do
    if [[ -f "$repo/.git/hooks/post-commit" ]] || \
       [[ -d "$repo/.git/hooks" && $(ls "$repo/.git/hooks/" 2>/dev/null | grep -c "git-ai" || true) -gt 0 ]]; then
        ((HOOK_OK++)) || true
    fi
done

if [[ $HOOK_OK -gt 0 ]]; then
    success "hooks 安装正常（已在 $HOOK_OK 个仓库检测到）"
else
    warn "未检测到 hooks，请手动在项目根目录运行：git-ai install-hooks"
fi

# ─── 7. 完成提示 ──────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}✅ 配置完成！${NC}"
echo ""
echo "接下来你只需要正常写代码、使用 AI 工具、git commit。"
echo "AI 数据会自动记录，每次 git push 时自动上报到团队看板。"
echo ""
if [ -n "$METRICS_SERVER" ]; then
    echo -e "团队看板：${BLUE}${METRICS_SERVER}${NC}"
    echo ""
fi
echo "查看本地 AI 统计："
echo "  git-ai stats"
echo ""
echo "手动触发上报（调试用）："
echo "  git-ai upload-metrics --verbose"
echo ""
