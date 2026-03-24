-- ─────────────────────────────────────────────────────────────────────────────
-- git-ai Team Dashboard · Database Schema
-- ─────────────────────────────────────────────────────────────────────────────

-- ── 扩展 ────────────────────────────────────────────────────────────────────
CREATE EXTENSION IF NOT EXISTS pg_trgm;  -- 文件路径模糊搜索

-- ── 仓库注册表 ───────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS repos (
    name        TEXT PRIMARY KEY,
    url         TEXT NOT NULL,
    created_at  TIMESTAMPTZ DEFAULT NOW()
);

-- ── Collector 状态（增量采集断点） ────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS collector_state (
    repo_name               TEXT PRIMARY KEY REFERENCES repos(name),
    last_collected_at       TIMESTAMPTZ,
    last_commit_sha         TEXT,
    total_commits_processed INT  DEFAULT 0,
    last_error              TEXT,
    last_error_at           TIMESTAMPTZ
);

-- ── 每个 commit 的 AI 统计（聚合级） ─────────────────────────────────────────
CREATE TABLE IF NOT EXISTS commit_metrics (
    id                    SERIAL PRIMARY KEY,
    repo_name             TEXT    NOT NULL REFERENCES repos(name),
    commit_sha            TEXT    NOT NULL,
    commit_author_email   TEXT,
    commit_author_name    TEXT,
    committed_at          TIMESTAMPTZ,
    collected_at          TIMESTAMPTZ DEFAULT NOW(),
    schema_version        TEXT,
    git_ai_version        TEXT,

    -- 统计字段（来自 prompt 记录的聚合）
    prompt_count          INT DEFAULT 0,
    total_ai_additions    INT DEFAULT 0,   -- AI 生成的总行数
    total_ai_deletions    INT DEFAULT 0,   -- AI 删除的总行数
    accepted_lines        INT DEFAULT 0,   -- 未经人工修改直接 commit 的 AI 行
    overridden_lines      INT DEFAULT 0,   -- 人工修改后 commit 的 AI 行

    -- 工具/模型信息（JSON 数组）
    tools_used            JSONB,           -- ["cursor", "claude-code"]
    models_used           JSONB,           -- ["claude-sonnet-4-5"]

    UNIQUE (repo_name, commit_sha)
);

-- ── 每个 prompt session 的明细 ────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS prompt_metrics (
    id                    SERIAL PRIMARY KEY,
    repo_name             TEXT NOT NULL,
    commit_sha            TEXT NOT NULL,
    prompt_hash           TEXT NOT NULL,   -- 16 位 session hash
    tool                  TEXT,            -- agent_id.tool
    model                 TEXT,            -- agent_id.model
    human_author_email    TEXT,

    total_additions       INT DEFAULT 0,   -- AI 生成总行
    total_deletions       INT DEFAULT 0,
    accepted_lines        INT DEFAULT 0,   -- 直接接受行
    overridden_lines      INT DEFAULT 0,   -- 人工修改行
    collected_at          TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE (repo_name, commit_sha, prompt_hash),
    FOREIGN KEY (repo_name, commit_sha)
        REFERENCES commit_metrics(repo_name, commit_sha) ON DELETE CASCADE
);

-- ── 每个文件的 AI 归属行数 ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS file_metrics (
    id                    SERIAL PRIMARY KEY,
    repo_name             TEXT NOT NULL,
    commit_sha            TEXT NOT NULL,
    file_path             TEXT NOT NULL,
    attributed_lines      INT  DEFAULT 0,  -- attestation 中统计的 AI 归属行数
    prompt_hashes         JSONB,           -- 涉及该文件的 session hash 列表
    collected_at          TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE (repo_name, commit_sha, file_path),
    FOREIGN KEY (repo_name, commit_sha)
        REFERENCES commit_metrics(repo_name, commit_sha) ON DELETE CASCADE
);

-- ── 索引 ─────────────────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_cm_committed_at       ON commit_metrics (committed_at);
CREATE INDEX IF NOT EXISTS idx_cm_repo_time          ON commit_metrics (repo_name, committed_at);
CREATE INDEX IF NOT EXISTS idx_cm_author             ON commit_metrics (commit_author_email);
CREATE INDEX IF NOT EXISTS idx_pm_tool               ON prompt_metrics (tool);
CREATE INDEX IF NOT EXISTS idx_pm_model              ON prompt_metrics (model);
CREATE INDEX IF NOT EXISTS idx_pm_author             ON prompt_metrics (human_author_email);
CREATE INDEX IF NOT EXISTS idx_fm_repo_file          ON file_metrics   (repo_name, file_path);
-- 覆盖索引，加速 Grafana 时序聚合
CREATE INDEX IF NOT EXISTS idx_cm_covering ON commit_metrics
    (committed_at, repo_name, accepted_lines, total_ai_additions, commit_author_email);

-- ── 视图 ─────────────────────────────────────────────────────────────────────

-- 每日团队概览
CREATE OR REPLACE VIEW v_daily_summary AS
SELECT
    date_trunc('day', committed_at)                          AS day,
    repo_name,
    COUNT(*)                                                 AS total_commits,
    COUNT(*) FILTER (WHERE total_ai_additions > 0)           AS ai_commits,
    SUM(total_ai_additions)                                  AS ai_generated,
    SUM(accepted_lines)                                      AS ai_accepted,
    SUM(overridden_lines)                                    AS ai_overridden,
    ROUND(
        SUM(accepted_lines)::numeric /
        NULLIF(SUM(total_ai_additions), 0) * 100, 1
    )                                                        AS acceptance_rate,
    ROUND(
        SUM(total_ai_additions)::numeric /
        NULLIF(SUM(total_ai_additions + (
            SELECT COALESCE(SUM(git_diff_added_lines), 0)
        )), 0) * 100, 1
    )                                                        AS ai_pct
FROM commit_metrics
GROUP BY 1, 2;

-- 工具/模型周报
CREATE OR REPLACE VIEW v_tool_model_weekly AS
SELECT
    date_trunc('week', cm.committed_at)                      AS week,
    pm.tool,
    pm.model,
    cm.repo_name,
    COUNT(DISTINCT cm.commit_sha)                            AS commits,
    SUM(pm.total_additions)                                  AS generated_lines,
    SUM(pm.accepted_lines)                                   AS accepted_lines,
    ROUND(
        SUM(pm.accepted_lines)::numeric /
        NULLIF(SUM(pm.total_additions), 0) * 100, 1
    )                                                        AS acceptance_rate
FROM prompt_metrics pm
JOIN commit_metrics cm USING (repo_name, commit_sha)
GROUP BY 1, 2, 3, 4;

-- 开发者周报
CREATE OR REPLACE VIEW v_dev_weekly AS
SELECT
    date_trunc('week', committed_at)                         AS week,
    commit_author_email                                      AS developer,
    repo_name,
    COUNT(*)                                                 AS commits,
    SUM(total_ai_additions)                                  AS ai_generated,
    SUM(accepted_lines)                                      AS ai_accepted,
    ROUND(
        SUM(accepted_lines)::numeric /
        NULLIF(SUM(total_ai_additions), 0) * 100, 1
    )                                                        AS acceptance_rate,
    tools_used
FROM commit_metrics
GROUP BY 1, 2, 3, tools_used;

-- 高 AI 密度文件榜
CREATE OR REPLACE VIEW v_hot_files AS
SELECT
    repo_name,
    file_path,
    SUM(attributed_lines)        AS total_ai_lines,
    COUNT(DISTINCT commit_sha)   AS touched_by_commits
FROM file_metrics
GROUP BY repo_name, file_path
ORDER BY total_ai_lines DESC;

-- ── Grafana 只读用户 ──────────────────────────────────────────────────────────
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'grafana_ro') THEN
        CREATE ROLE grafana_ro LOGIN PASSWORD 'grafana_local_pass';
    END IF;
END
$$;

GRANT CONNECT ON DATABASE gitai_dashboard TO grafana_ro;
GRANT USAGE   ON SCHEMA public TO grafana_ro;
GRANT SELECT  ON ALL TABLES    IN SCHEMA public TO grafana_ro;
GRANT SELECT  ON ALL SEQUENCES IN SCHEMA public TO grafana_ro;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO grafana_ro;
