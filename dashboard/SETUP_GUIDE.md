# 团队 AI 代码看板 · 部署手册

## 你需要做的事，总览

```
第一步  服务端部署（你，一次性，30 分钟）
第二步  仓库配置（你，每个仓库执行一条命令）
第三步  开发者配置（每个团队成员，5 分钟）
第四步  日常维护（按需）
```

---

## 第一步：服务端部署

> 需要一台能访问团队 git 仓库的 Linux/Mac 服务器，安装了 Docker。

### 1.1 把代码放到服务器上

```bash
# 如果你在 git-ai 仓库里
scp -r ./dashboard user@your-server:/opt/gitai-dashboard

# 或者直接在服务器上操作
ssh user@your-server
mkdir -p /opt/gitai-dashboard
# 把 dashboard/ 目录的内容传过去
```

### 1.2 创建配置文件

```bash
cd /opt/gitai-dashboard

# 从模板复制
cp .env.example .env
cp config/repos.example.yaml config/repos.yaml
```

### 1.3 编辑 `.env`，填入你的密码和 Token

```bash
vim .env
```

需要修改的内容：

```dotenv
# 数据库密码（随便设，记住就行）
POSTGRES_PASSWORD=替换成强密码

# Grafana 数据库只读账号密码
GRAFANA_DB_PASSWORD=替换成另一个密码

# Grafana 管理员密码（登录看板用）
GF_SECURITY_ADMIN_PASSWORD=替换成管理员密码

# 如果仓库用 HTTPS Token 认证，填在这里
# GitHub 生成方式：GitHub → Settings → Developer settings → Personal access tokens
# 权限只需要：Contents: Read-only
GITHUB_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxx

# GitLab 生成方式：User Settings → Access Tokens
# 权限只需要：read_repository
# GITLAB_TOKEN=glpat-xxxxxxxxxxxxxxxxxxxx
```

### 1.4 编辑 `config/repos.yaml`，填入你的仓库列表

```bash
vim config/repos.yaml
```

**SSH 认证示例（推荐）：**
```yaml
repos:
  - name: backend
    url: git@github.com:your-org/backend.git
    auth: ssh

  - name: frontend
    url: git@github.com:your-org/frontend.git
    auth: ssh

  - name: mobile
    url: git@github.com:your-org/mobile.git
    auth: ssh
```

**HTTPS Token 认证示例：**
```yaml
repos:
  - name: backend
    url: https://github.com/your-org/backend.git
    auth: token
    token_env: GITHUB_TOKEN      # 对应 .env 里的变量名
```

**混用示例：**
```yaml
repos:
  - name: backend
    url: git@github.com:your-org/backend.git
    auth: ssh

  - name: frontend
    url: https://gitlab.com/your-org/frontend.git
    auth: token
    token_env: GITLAB_TOKEN
```

### 1.5 配置 SSH 私钥（如使用 SSH 认证）

```bash
mkdir -p config/credentials

# 把能访问所有仓库的 SSH 私钥复制进来
cp ~/.ssh/id_rsa config/credentials/id_rsa
chmod 600 config/credentials/id_rsa

# 如果不同仓库用不同 key，可以都放进来
# cp ~/.ssh/id_rsa_work config/credentials/id_rsa_work
```

> 如果服务器上没有 SSH key，需要先生成并添加到 GitHub/GitLab：
> ```bash
> ssh-keygen -t ed25519 -C "gitai-collector" -f config/credentials/id_rsa
> cat config/credentials/id_rsa.pub  # 把公钥添加到 GitHub/GitLab 的 Deploy Keys
> ```

### 1.6 处理数据库初始化中的 grafana 密码

`schema.sql` 中创建 `grafana_ro` 用户时需要读取密码，需要在启动前做一步替换：

```bash
# 将 schema.sql 中的占位符替换为实际密码
# 先读取你在 .env 里设置的 GRAFANA_DB_PASSWORD
source .env
sed -i "s/:'grafana_password'/'${GRAFANA_DB_PASSWORD}'/" database/schema.sql
```

### 1.7 启动所有服务

```bash
docker compose up -d
```

查看启动状态：
```bash
docker compose ps
```

正常输出：
```
NAME                STATUS
collector           running
grafana             running
postgres            running (healthy)
```

查看采集日志（首次会克隆仓库，可能需要几分钟）：
```bash
docker compose logs -f collector
```

正常日志示例：
```
[info] 数据库连接池初始化完成
[info] Collector 启动，采集间隔 300s
[info] [backend] 首次克隆 git@github.com:your-org/backend.git
[info] [backend] 克隆完成
[info] [backend] 待处理 42 个 commit（共 42 个有 notes）
[info] [backend] 完成，本次处理 42 个 commit
```

### 1.8 访问 Grafana

浏览器打开：`http://your-server:3000`

- 账号：`admin`
- 密码：你在 `.env` 里设置的 `GF_SECURITY_ADMIN_PASSWORD`

进入后点击左侧 Dashboards → Team AI Code Dashboard，看板自动加载。

---

## 第二步：配置每个仓库推送 git notes

> 这一步确保开发者 push 代码时，AI 数据（git notes）也一起推送到远端。

**在每个需要追踪的仓库根目录执行：**

```bash
git config --add remote.origin.push "refs/notes/ai:refs/notes/ai"
git config --add remote.origin.fetch "+refs/notes/ai:refs/notes/ai"
```

**批量脚本（如果你管理所有仓库）：**

```bash
# 假设所有仓库都在 ~/Code 目录下
for repo in ~/Code/*/; do
  if [ -d "$repo/.git" ]; then
    echo "配置 $repo"
    git -C "$repo" config --add remote.origin.push "refs/notes/ai:refs/notes/ai"
    git -C "$repo" config --add remote.origin.fetch "+refs/notes/ai:refs/notes/ai"
  fi
done
```

**也可以推送到 CI/CD 里**，在 GitHub Actions / GitLab CI 中加一步：

```yaml
# GitHub Actions 示例：.github/workflows/push-notes.yml
name: Push git-ai notes
on: [push]
jobs:
  push-notes:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - name: Fetch and push git-ai notes
        run: |
          git fetch origin refs/notes/ai:refs/notes/ai || true
          git push origin refs/notes/ai:refs/notes/ai || true
```

```yaml
# GitLab CI 示例：.gitlab-ci.yml 中加一个 job
push-ai-notes:
  stage: deploy
  script:
    - git fetch origin refs/notes/ai:refs/notes/ai || true
    - git push origin refs/notes/ai:refs/notes/ai || true
  only:
    - main
    - master
```

---

## 第三步：开发者配置（每人执行一次）

> 发给每个团队成员，让他们在自己的电脑上执行。

### 方式一：一键脚本（最简单）

```bash
bash scripts/developer-setup.sh
```

脚本会自动：
1. 安装 git-ai（如果还没装）
2. 在找到的所有 git 仓库安装 hooks
3. 配置 git notes 自动推送

### 方式二：手动（3 条命令）

```bash
# 1. 安装 git-ai
curl -fsSL https://usegitai.com/install.sh | bash

# 2. 在每个项目目录安装 hooks（让 AI 工具操作被自动记录）
cd your-project
git-ai install-hooks

# 3. 配置 git notes 自动推送
git config --add remote.origin.push "refs/notes/ai:refs/notes/ai"
git config --add remote.origin.fetch "+refs/notes/ai:refs/notes/ai"
```

### 验证是否成功

```bash
# 用 AI 工具写一段代码，然后 commit
git add . && git commit -m "test ai tracking"

# 检查是否有 notes 数据
git notes --ref=ai list
# 有输出（commit SHA）说明成功

# 查看 notes 内容
git notes --ref=ai show HEAD

# 本地统计
git-ai stats
```

### 开发者日常使用

配置完成后，**不需要改变任何工作习惯**：
- 正常用 Cursor / Claude Code / Copilot 等工具写代码
- 正常 git commit
- 正常 git push（notes 会自动推送）

看板数据在下次 Collector 采集后（最多 5 分钟）更新。

---

## 第四步：日常维护

### 新增仓库

1. 编辑服务器上的 `config/repos.yaml`，追加新仓库
2. 重启 Collector：
   ```bash
   docker compose restart collector
   ```
3. 在新仓库执行第二步的 notes 推送配置

### 新增开发者

让新同学执行第三步即可，历史 commit 中已有的 notes 会在下次采集时自动入库。

### 查看采集状态

```bash
# 实时日志
docker compose logs -f collector

# 采集状态（在 Grafana 里也有 "Collector 采集状态" 面板）
docker compose exec postgres psql -U gitai -d gitai_dashboard \
  -c "SELECT repo_name, last_collected_at, total_commits_processed, last_error FROM collector_state;"
```

### 停止 / 重启服务

```bash
docker compose stop        # 停止
docker compose start       # 启动
docker compose restart     # 重启
docker compose down        # 停止并删除容器（数据保留在 volume 里）
```

### 数据备份

```bash
# 备份数据库
docker compose exec postgres pg_dump -U gitai gitai_dashboard > backup_$(date +%Y%m%d).sql

# 恢复
cat backup_20240101.sql | docker compose exec -T postgres psql -U gitai gitai_dashboard
```

### 升级

```bash
git pull  # 更新代码
docker compose build collector  # 重新构建 collector 镜像
docker compose up -d             # 重启服务
```

---

## 看板功能说明

| 面板 | 说明 |
|------|------|
| AI 生成行数 | 时间段内 AI 工具生成的总代码行数 |
| 直接接受行数 | 开发者未修改直接 commit 的 AI 代码行数 |
| AI 接受率 | 接受行 / 生成行，反映 AI 代码质量 |
| 活跃开发者数 | 时间段内有 AI 辅助 commit 的开发者数量 |
| 每日 AI 代码量 | 按仓库分组的每日趋势，可筛选仓库 |
| AI 接受率趋势 | 接受率随时间的变化，反映 AI 工具调整效果 |
| 工具使用分布 | Cursor / Claude Code / Copilot 各占比 |
| 模型接受率对比 | 哪个模型产出的代码被接受率最高 |
| 开发者排行 | 个人 AI 产出量和接受率（可用于了解 AI 工具采用情况）|
| 仓库概览 | 各仓库 AI 使用汇总 |
| AI 高密度文件 | 哪些文件有最多 AI 代码（可用于 review 优先级参考）|
| Collector 状态 | 每个仓库的采集时间和错误状态 |

---

## 常见问题

**Q: 数据库初始化失败，提示 grafana_ro 创建失败？**

```bash
# 手动连入数据库修复
source .env
docker compose exec postgres psql -U gitai -d gitai_dashboard \
  -c "CREATE ROLE grafana_ro LOGIN PASSWORD '${GRAFANA_DB_PASSWORD}';"
docker compose exec postgres psql -U gitai -d gitai_dashboard \
  -c "GRANT CONNECT ON DATABASE gitai_dashboard TO grafana_ro;"
docker compose exec postgres psql -U gitai -d gitai_dashboard \
  -c "GRANT USAGE ON SCHEMA public TO grafana_ro;"
docker compose exec postgres psql -U gitai -d gitai_dashboard \
  -c "GRANT SELECT ON ALL TABLES IN SCHEMA public TO grafana_ro;"
```

**Q: Collector 报 SSH 连接失败？**

```bash
# 在 collector 容器内测试连通性
docker compose exec collector ssh -T git@github.com -i /root/.ssh/id_rsa
# 预期输出：Hi username! You've successfully authenticated...
```

确认公钥已添加到 GitHub → Settings → SSH and GPG Keys，或仓库的 Deploy Keys。

**Q: 看板没有数据？**

按以下顺序排查：
1. `docker compose logs collector` 确认采集没有报错
2. `docker compose exec postgres psql -U gitai -d gitai_dashboard -c "SELECT COUNT(*) FROM commit_metrics;"` 确认数据库有数据
3. Grafana 右上角时间范围是否覆盖有数据的时间段
4. 开发者是否已安装 hooks 并使用 AI 工具 commit 过

**Q: git notes --ref=ai list 没有输出？**

说明该仓库还没有 AI 数据。确认：
1. `git-ai install-hooks` 已执行
2. 用 AI 工具（Cursor/Claude Code 等）写了代码
3. 执行了 git commit（不是仅 git add）

**Q: 想追踪某个仓库的历史数据？**

Collector 会自动处理仓库中所有有 git notes 的历史 commit，无需额外操作。
前提是这些历史 notes 已经 push 到远端（即开发者之前已配置了 notes 推送）。

---

## 检查清单

### 服务端
- [ ] Docker 和 Docker Compose 已安装
- [ ] `.env` 文件已创建并填入密码
- [ ] `config/repos.yaml` 已创建并填入仓库列表
- [ ] SSH 私钥放入 `config/credentials/`（如使用 SSH 认证）
- [ ] `schema.sql` 中 grafana 密码已替换（第 1.6 步）
- [ ] `docker compose up -d` 启动成功
- [ ] 能访问 `http://your-server:3000`

### 每个仓库
- [ ] 已配置 `refs/notes/ai` 推送（第二步）

### 每个开发者
- [ ] `git-ai` 已安装
- [ ] `git-ai install-hooks` 已在项目仓库执行
- [ ] git notes 推送已配置
- [ ] 测试：用 AI 工具写代码 → commit → 确认 `git notes --ref=ai list` 有输出
