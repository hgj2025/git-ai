# git-ai Team Dashboard

团队 AI 代码产出自建看板。从所有仓库的 git notes 中采集数据，存入 PostgreSQL，通过 Grafana 可视化。

```
开发者机器                          团队 Git 服务器
┌──────────────────────┐           ┌─────────────────────┐
│  AI 工具写代码         │           │  refs/notes/ai      │
│  git commit          │──push──>  │  (git notes)        │
│  git push + notes    │           └────────┬────────────┘
└──────────────────────┘                    │ git fetch
                                 ┌──────────▼────────────┐
                                 │  Collector (Python)   │
                                 │  每 5 分钟增量采集     │
                                 └──────────┬────────────┘
                                 ┌──────────▼────────────┐
                                 │  PostgreSQL            │
                                 └──────────┬────────────┘
                                 ┌──────────▼────────────┐
                                 │  Grafana Dashboard     │
                                 │  :3000                 │
                                 └────────────────────────┘
```

## 看板预览

- **概览指标**：AI 生成行、接受率、活跃开发者、AI Commits
- **趋势图**：每日 AI 代码量、接受率变化
- **工具分布**：Claude Code / Cursor / Copilot / Windsurf 占比
- **模型对比**：各模型接受率排行
- **开发者排行**：个人 AI 产出和接受率
- **仓库对比**：跨仓库统计
- **文件榜**：AI 高密度文件 Top 20
- **采集状态**：Collector 健康监控

---

## 快速部署（服务端）

### 前置条件

- Docker + Docker Compose v2
- 能访问团队 git 仓库的 SSH 私钥或 HTTPS Token

### 1. 配置

```bash
cd dashboard/

# 复制配置文件
cp .env.example .env
cp config/repos.example.yaml config/repos.yaml
```

编辑 `.env`，修改密码：
```bash
POSTGRES_PASSWORD=your_strong_password
GRAFANA_DB_PASSWORD=your_grafana_password
GF_SECURITY_ADMIN_PASSWORD=your_admin_password
GITHUB_TOKEN=ghp_xxxxxxxxxxxx     # 如使用 HTTPS Token 认证
```

编辑 `config/repos.yaml`，填入你的仓库列表：
```yaml
repos:
  - name: backend
    url: git@github.com:your-org/backend.git
    auth: ssh

  - name: frontend
    url: https://github.com/your-org/frontend.git
    auth: token
    token_env: GITHUB_TOKEN
```

### 2. 配置 SSH Key（如使用 SSH 认证）

```bash
mkdir -p config/credentials
# 将能访问仓库的 SSH 私钥放入此目录
cp ~/.ssh/id_rsa config/credentials/id_rsa
chmod 600 config/credentials/id_rsa
```

### 3. 启动

```bash
docker compose up -d
```

查看日志：
```bash
docker compose logs -f collector    # 采集日志
docker compose logs -f postgres     # 数据库日志
```

访问 Grafana：http://your-server:3000
- 默认账号：admin / 你在 .env 中设置的密码

### 4. 等待数据

Collector 首次运行会克隆所有仓库并处理历史数据，时间取决于仓库大小和 commit 数量。
可通过日志监控进度：
```bash
docker compose logs -f collector | grep "处理"
```

---

## 开发者配置

### 方式一：一键脚本（推荐）

将 `scripts/developer-setup.sh` 发布到内网，让每个开发者运行：

```bash
# 发布到团队内网后
curl -fsSL https://your-server/developer-setup.sh | bash

# 或本地执行
bash scripts/developer-setup.sh
```

脚本会自动：
1. 安装 git-ai（如未安装）
2. 在找到的所有 git 仓库安装 hooks
3. 配置 git notes 自动推送

### 方式二：手动配置

每个开发者在各自仓库执行：

```bash
# 1. 安装 git-ai
curl -fsSL https://usegitai.com/install.sh | bash

# 2. 安装 AI 工具 hooks（在每个项目目录执行）
git-ai install-hooks

# 3. 配置 git notes 自动推送（在每个项目目录执行）
git config --add remote.origin.push "refs/notes/ai:refs/notes/ai"
git config --add remote.origin.fetch "+refs/notes/ai:refs/notes/ai"
```

之后正常使用 AI 工具写代码、git commit、git push 即可，数据自动采集。

### 支持的 AI 工具

git-ai 支持自动追踪以下工具的代码贡献：

| 工具 | 说明 |
|------|------|
| Claude Code | Anthropic Claude CLI |
| Cursor | AI 代码编辑器 |
| GitHub Copilot | GitHub 助手 |
| Windsurf | Codeium IDE |
| Gemini | Google AI |
| Codex | OpenAI |

---

## 目录结构

```
dashboard/
├── docker-compose.yml          # 一键启动所有服务
├── .env.example                # 环境变量模板
│
├── collector/                  # 数据采集服务（Python）
│   ├── main.py                 # 主循环
│   ├── config.py               # 配置加载
│   ├── repo_manager.py         # git 仓库操作
│   ├── notes_parser.py         # git notes 解析
│   ├── db.py                   # 数据库写入
│   ├── Dockerfile
│   └── requirements.txt
│
├── database/
│   └── schema.sql              # 表结构 + 视图 + 只读用户
│
├── grafana/
│   └── provisioning/           # Grafana 自动配置
│       ├── datasources/        # PostgreSQL 数据源
│       └── dashboards/         # 看板 JSON
│
├── scripts/
│   └── developer-setup.sh     # 开发者一键配置脚本
│
└── config/
    ├── repos.yaml              # 仓库列表（需自行创建）
    └── credentials/            # SSH 私钥目录（需自行放置）
```

---

## 核心指标说明

| 指标 | 定义 |
|------|------|
| AI 生成行 (`total_ai_additions`) | AI 在该 session 中生成的总行数 |
| 直接接受行 (`accepted_lines`) | 无修改直接进入 commit 的 AI 行 |
| 修改后接受行 (`overridden_lines`) | 人工修改后 commit 的 AI 行 |
| 接受率 | `accepted_lines / total_ai_additions × 100%` |
| AI Commit | 有 git-ai notes 的 commit |

接受率高（>65%）说明 AI 代码质量好，开发者信任度高。
接受率低（<40%）说明需要更多交互调整，或提示词需优化。

---

## 常见问题

**Q: Collector 报 "克隆失败" 怎么办？**

检查 SSH 私钥权限和仓库访问权限：
```bash
docker compose exec collector ssh -T git@github.com
```

**Q: git notes 没有数据？**

确认开发者已：
1. 安装 git-ai 并运行 `git-ai install-hooks`
2. 使用 AI 工具写了代码并 commit
3. 在 git push 时推送了 notes

手动验证：`git notes --ref=ai list`（有输出则表示有数据）

**Q: 如何追加新仓库？**

编辑 `config/repos.yaml` 添加新仓库，重启 collector：
```bash
docker compose restart collector
```

**Q: 数据量大，初次采集很慢？**

正常，历史数据只处理一次。可调整并发（目前单线程保守设计），或接受等待。
