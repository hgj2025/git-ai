#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# git-ai Server 一键部署
#
# 用法：
#   curl -fsSL https://raw.githubusercontent.com/hgj2025/git-ai/main/dashboard/scripts/server-setup.sh | bash
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

REPO="https://github.com/hgj2025/git-ai"
INSTALL_DIR="/opt/git-ai"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'
RED='\033[0;31m'; BOLD='\033[1m'; NC='\033[0m'

info()    { echo -e "${BLUE}[info]${NC} $*"; }
success() { echo -e "${GREEN}[ok]${NC} $*"; }
warn()    { echo -e "${YELLOW}[warn]${NC} $*"; }
error()   { echo -e "${RED}[error]${NC} $*" >&2; }
step()    { echo -e "\n${BOLD}▶ $*${NC}"; }

echo ""
echo -e "${BOLD}git-ai Server 一键部署${NC}"
echo ""

# ─── 1. 检查 Go 环境 ─────────────────────────────────────────────────────
step "检查环境"

if command -v go &>/dev/null; then
    success "Go $(go version | awk '{print $3}')"
else
    error "需要 Go 1.22+：https://go.dev/dl/"
    exit 1
fi

# ─── 2. 编译 server ──────────────────────────────────────────────────────
step "编译 git-ai-server"

TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

info "克隆源码…"
git clone --depth 1 "$REPO.git" "$TMP_DIR/git-ai"

info "编译中…"
cd "$TMP_DIR/git-ai/dashboard/server"
if ! go build -o git-ai-server .; then
    error "编译失败"
    exit 1
fi

# ─── 3. 安装 ─────────────────────────────────────────────────────────────
step "安装"

sudo mkdir -p "$INSTALL_DIR"
sudo cp git-ai-server "$INSTALL_DIR/git-ai-server"
sudo chmod +x "$INSTALL_DIR/git-ai-server"
success "已安装到 $INSTALL_DIR/git-ai-server"

# ─── 4. 配置服务 ─────────────────────────────────────────────────────────
if [[ "$(uname -s)" == "Linux" ]] && command -v systemctl &>/dev/null; then
    step "配置 systemd 服务"

    sudo tee /etc/systemd/system/git-ai-server.service >/dev/null <<UNIT
[Unit]
Description=git-ai Metrics Server
After=network.target

[Service]
ExecStart=$INSTALL_DIR/git-ai-server
Restart=on-failure
WorkingDirectory=$INSTALL_DIR

[Install]
WantedBy=multi-user.target
UNIT

    sudo systemctl daemon-reload
    sudo systemctl enable --now git-ai-server
    success "服务已启动"
else
    step "启动方式"
    echo ""
    echo "  $INSTALL_DIR/git-ai-server"
    echo ""
    info "macOS 可使用 launchd 或直接后台运行：nohup ... &"
fi

# ─── 完成 ──────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}Server 部署完成！${NC}"
echo ""
echo -e "  看板地址：${BLUE}http://localhost:8080${NC}"
echo ""
echo "开发者安装命令（发给团队成员）："
echo -e "  ${BLUE}curl -fsSL https://raw.githubusercontent.com/hgj2025/git-ai/main/dashboard/scripts/developer-setup.sh | bash${NC}"
echo ""
