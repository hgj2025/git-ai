# git-ai

开源 Git 扩展，自动追踪 AI 生成的代码。每次提交自动记录哪些代码由 AI 编写、使用了什么模型和工具。

```
git commit
[main 0afe44b] feat: add retry logic
 2 files changed, 81 insertions(+), 3 deletions(-)
you  ██░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ ai
     6%             mixed   2%             92%
```

---

## 架构

```
开发者机器                          团队服务器（可选）
┌─────────────────────┐            ┌─────────────────────┐
│  git-ai CLI (Rust)  │──push───▶  │  git-ai-server (Go) │
│  · git hooks        │            │  · 接收上报数据      │
│  · AI 归因追踪      │            │  · Dashboard 看板    │
│  · 数据上报         │            │  · SQLite 存储       │
└─────────────────────┘            └─────────────────────┘
```

| 组件 | 角色 | 说明 |
|------|------|------|
| **CLI** (`git-ai`) | 开发者 | 安装后无感使用，自动追踪 AI 代码归因 |
| **Server** (`git-ai-server`) | 管理员 | 可选部署，提供团队级看板和统计 |

---

## 支持的 AI 编辑器

Claude Code · Codex · Cursor · VS Code · GitHub Copilot · Windsurf · Amp · Gemini · OpenCode · JetBrains · Droid

---

## 安装

### 开发者（CLI）

在项目目录下执行：

```bash
curl -fsSL https://raw.githubusercontent.com/hgj2025/git-ai/main/dashboard/scripts/developer-setup.sh | bash
```

脚本会自动：
- 首次运行：编译安装 CLI + 配置 hooks + 设置上报
- 再次运行（新仓库）：跳过编译，只安装 hooks

指定上报地址：

```bash
curl ... | bash -s -- --server http://your-server:8080
```

### 管理员（Server）

```bash
curl -fsSL https://raw.githubusercontent.com/hgj2025/git-ai/main/dashboard/scripts/server-setup.sh | bash
```

部署完成后访问 `http://localhost:8080` 查看看板。

---

## 核心功能

### AI 归因

每次 `git commit` 自动记录 AI 代码比例，数据存储在 Git Notes 中，不污染提交历史。

### AI Blame

`git blame` 的增强版，显示每行代码的 AI 归属：

```bash
git-ai blame src/main.rs
```

```
cb832b7 (dev    2025-12-13  133) pub fn execute_diff(
fe2c4c8 (claude 2025-12-02  138)     // Resolve commits to get from/to SHAs
fe2c4c8 (claude 2025-12-02  139)     let (from_commit, to_commit) = match spec {
```

### 统计

```bash
git-ai stats
```

### 数据上报

配置后每次 `git push` 自动上报指标到团队服务器，不影响 push 速度。

```bash
# 查看配置
git config --global git-ai.metrics-server

# 手动调试
git-ai upload-metrics --verbose
```

---

## 工作原理

1. AI 编辑器通过 hooks 报告代码变更
2. git-ai 将每次编辑记录为 checkpoint（`.git/ai/` 下的小 diff）
3. `git commit` 时，git-ai 将所有 checkpoint 合并为 Authorship Log，通过 Git Notes 附加到提交
4. 归因数据在 rebase、merge、squash、cherry-pick 时自动保留

数据格式详见 [Git AI Standard v3.0.0](specs/git_ai_standard_v3.0.0.md)。

---

## 卸载

### CLI（一键卸载）

```bash
curl -fsSL https://raw.githubusercontent.com/hgj2025/git-ai/main/dashboard/scripts/uninstall.sh | bash
```

自动清理：hooks、全局 git 配置、shell PATH、安装目录。

### Server

```bash
# Linux
systemctl disable --now git-ai-server
rm /etc/systemd/system/git-ai-server.service
rm -rf /opt/git-ai
```

---

## License

Apache 2.0
