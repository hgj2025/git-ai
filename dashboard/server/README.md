# git-ai 团队 AI 看板

> 项目地址：https://github.com/hgj2025/git-ai

轻量统计服务：开发者每次 `git push` 后自动上报 AI 指标，Go 单二进制 + SQLite 存储，内嵌 Dashboard，无需 Docker/PostgreSQL/Grafana。

---

## 服务端：一键部署

### 方式一：从源码构建（需要 Go 1.22+）

```bash
git clone https://github.com/hgj2025/git-ai.git
cd git-ai/dashboard/server
go build -o git-ai-server .
./git-ai-server --port 8080 --db /data/metrics.db --token your-secret
```

### 方式二：下载预编译二进制

```bash
curl -fsSL https://github.com/hgj2025/git-ai/releases/latest/download/git-ai-server-$(uname -s)-$(uname -m) \
  -o git-ai-server && chmod +x git-ai-server
./git-ai-server --port 8080 --db /data/metrics.db --token your-secret
```

### 启动参数

| 参数 | 环境变量 | 默认值 | 说明 |
|------|----------|--------|------|
| `--port` | `GIT_AI_SERVER_PORT` | `8080` | 监听端口 |
| `--db` | `GIT_AI_SERVER_DB` | `metrics.db` | SQLite 文件路径 |
| `--token` | `GIT_AI_SERVER_TOKEN` | 空（无认证） | Bearer Token，非空时校验 |

### 后台运行（systemd）

```ini
# /etc/systemd/system/git-ai-server.service
[Unit]
Description=git-ai Metrics Server
After=network.target

[Service]
ExecStart=/opt/git-ai/git-ai-server --port 8080 --db /opt/git-ai/metrics.db --token your-secret
Restart=on-failure
WorkingDirectory=/opt/git-ai

[Install]
WantedBy=multi-user.target
```
```bash
systemctl enable --now git-ai-server
```

### 访问看板
```
http://your-server:8080
```

---

## 开发者侧：一键安装 / 卸载

服务启动后，把以下命令发给团队成员，运行一次即完成全部配置：

```bash
# 一键安装（将 your-server:8080 替换为实际部署地址）
curl -fsSL http://your-server:8080/install.sh | bash

# 一键卸载
curl -fsSL http://your-server:8080/uninstall.sh | bash
```

> `install.sh` 由服务端动态生成，自动将服务地址和 Token 注入脚本，开发者无需手动填写任何参数。

安装完成后，每次 `git push` 会自动在后台上报 AI 指标，不影响 push 速度。

### 本地测试（服务跑在本机）

```bash
# 安装
curl -fsSL http://localhost:8080/install.sh | bash

# 卸载
curl -fsSL http://localhost:8080/uninstall.sh | bash
```

### 手动验证
```bash
# 检查配置
git config --global git-ai.metrics-server

# 手动触发上报（调试用）
git-ai upload-metrics --verbose

# 查看看板
open http://localhost:8080
```

---

## 卸载

### 开发者侧卸载
```bash
# 1. 清除服务配置
git config --global --unset git-ai.metrics-server
git config --global --unset git-ai.metrics-token

# 2. 重新安装 hooks（会覆盖 post-push hook）
git-ai install-hooks

# 或彻底卸载 git-ai hooks（在各仓库目录执行）
git-ai uninstall-hooks
```

### 服务端卸载
```bash
# systemd
systemctl disable --now git-ai-server
rm /etc/systemd/system/git-ai-server.service

# 删除数据（可选）
rm /opt/git-ai/metrics.db
rm /opt/git-ai/git-ai-server
```

---

## API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/` | GET | Dashboard HTML |
| `/api/stats?days=30&repo=all` | GET | 聚合统计 JSON |
| `/api/repos` | GET | 仓库列表 |
| `/api/report` | POST | 上报指标（需 Token） |

### POST /api/report 示例
```bash
curl -X POST http://localhost:8080/api/report \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-secret" \
  -d '{
    "repo": "my-project",
    "commits": [{
      "commit_sha": "abc123",
      "author_email": "dev@example.com",
      "author_name": "Dev",
      "committed_at": "2026-03-24T10:00:00Z",
      "prompts": [{
        "tool": "claude-code",
        "model": "claude-sonnet-4-6",
        "total_additions": 100,
        "accepted_lines": 80,
        "overridden_lines": 5
      }]
    }]
  }'
```
