"""
git-ai Team Dashboard · Collector
----------------------------------
主循环：定时从所有仓库采集 git notes，写入 PostgreSQL。
"""

import logging
import os
import time
from pathlib import Path

import db
import repo_manager as rm
from config import load_config, RepoConfig
from notes_parser import parse_note

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)
logger = logging.getLogger("collector")


# ─── 单个仓库的采集逻辑 ───────────────────────────────────────────────────────

def collect_repo(cfg: RepoConfig, repos_dir: str, conn) -> int:
    """
    采集单个仓库的新 AI commit 数据。
    返回本次处理的 commit 数量。
    """
    logger.info(f"[{cfg.name}] 开始采集")

    # 1. 确保本地 bare clone 存在并拉取最新数据
    try:
        local = rm.ensure_repo(cfg, repos_dir)
    except RuntimeError as e:
        db.set_error(conn, cfg.name, str(e))
        logger.error(f"[{cfg.name}] {e}")
        return 0

    # 2. 获取哪些 commit 有 notes
    noted_set = set(rm.list_noted_commits(local))
    if not noted_set:
        logger.info(f"[{cfg.name}] 暂无 git notes，跳过")
        return 0

    # 3. 获取上次断点 SHA，确定增量范围
    last_sha = db.get_last_sha(conn, cfg.name)
    new_commits = rm.new_commits_since(local, last_sha)

    # 4. 取交集：既有 notes 又是新 commit
    to_process = [sha for sha in new_commits if sha in noted_set]
    logger.info(f"[{cfg.name}] 待处理 {len(to_process)} 个 commit（共 {len(noted_set)} 个有 notes）")

    processed = 0
    last_processed_sha = last_sha

    for sha in to_process:
        try:
            _process_commit(cfg.name, sha, local, conn)
            last_processed_sha = sha
            processed += 1
        except Exception as e:
            logger.error(f"[{cfg.name}] 处理 commit {sha[:8]} 失败: {e}", exc_info=True)
            # 记录错误但继续处理后续 commit

    # 5. 更新采集状态
    if last_processed_sha and last_processed_sha != last_sha:
        db.update_state(conn, cfg.name, last_processed_sha, processed)

    logger.info(f"[{cfg.name}] 完成，本次处理 {processed} 个 commit")
    return processed


def _process_commit(repo_name: str, sha: str, local: Path, conn) -> None:
    """解析单个 commit 的 note 并写入数据库"""
    # 读取 note 文本
    note_text = rm.read_note(local, sha)
    if not note_text:
        return

    # 解析 git-ai authorship log
    parsed = parse_note(note_text)
    if not parsed:
        logger.debug(f"[{repo_name}] {sha[:8]} note 格式不识别，跳过")
        return

    # 读取 commit 元数据
    meta = rm.read_commit_metadata(local, sha)

    # ── 写入 commit_metrics ────────────────────────────────────────────────────
    tools = list({p.tool for p in parsed.prompts})
    models = list({p.model for p in parsed.prompts})

    commit_id = db.upsert_commit(conn, {
        "repo_name":          repo_name,
        "commit_sha":         sha,
        "author_email":       meta.get("author_email", ""),
        "author_name":        meta.get("author_name", ""),
        "committed_at":       meta.get("committed_at"),
        "schema_version":     parsed.schema_version,
        "git_ai_version":     parsed.git_ai_version,
        "prompt_count":       len(parsed.prompts),
        "total_ai_additions": sum(p.total_additions for p in parsed.prompts),
        "total_ai_deletions": sum(p.total_deletions for p in parsed.prompts),
        "accepted_lines":     sum(p.accepted_lines for p in parsed.prompts),
        "overridden_lines":   sum(p.overridden_lines for p in parsed.prompts),
        "tools_used":         tools,
        "models_used":        models,
    })

    if commit_id is None:
        return  # 已存在，跳过（ON CONFLICT DO NOTHING）

    # ── 写入 prompt_metrics ────────────────────────────────────────────────────
    db.upsert_prompts(conn, [
        {
            "repo_name":          repo_name,
            "commit_sha":         sha,
            "prompt_hash":        p.hash,
            "tool":               p.tool,
            "model":              p.model,
            "human_author_email": p.human_author,
            "total_additions":    p.total_additions,
            "total_deletions":    p.total_deletions,
            "accepted_lines":     p.accepted_lines,
            "overridden_lines":   p.overridden_lines,
        }
        for p in parsed.prompts
    ])

    # ── 写入 file_metrics ──────────────────────────────────────────────────────
    db.upsert_files(conn, [
        {
            "repo_name":       repo_name,
            "commit_sha":      sha,
            "file_path":       fa.file_path,
            "attributed_lines": fa.total_lines,
            "prompt_hashes":   [e.hash for e in fa.entries],
        }
        for fa in parsed.attestations
        if fa.total_lines > 0
    ])


# ─── 主循环 ───────────────────────────────────────────────────────────────────

def run_once(cfg_path: str = "/app/config/repos.yaml") -> None:
    cfg = load_config(cfg_path)

    with db.get_conn() as conn:
        for repo in cfg.repos:
            db.ensure_repo(conn, repo.name, repo.url)

        for repo in cfg.repos:
            try:
                collect_repo(repo, cfg.repos_dir, conn)
            except Exception as e:
                logger.error(f"[{repo.name}] 采集异常: {e}", exc_info=True)


def main() -> None:
    database_url = os.environ.get("DATABASE_URL", "")
    if not database_url:
        raise SystemExit("环境变量 DATABASE_URL 未设置")

    db.init_pool(database_url)
    interval = int(os.environ.get("COLLECT_INTERVAL_SECONDS", "300"))

    logger.info(f"Collector 启动，采集间隔 {interval}s")

    while True:
        try:
            run_once()
        except Exception as e:
            logger.error(f"采集轮次异常: {e}", exc_info=True)

        logger.info(f"等待 {interval}s 后进行下次采集…")
        time.sleep(interval)


if __name__ == "__main__":
    main()
