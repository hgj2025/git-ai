"""
数据库操作：连接管理、upsert 写入。
所有写操作幂等（ON CONFLICT DO NOTHING / DO UPDATE）。
"""

import json
import logging
from contextlib import contextmanager
from typing import Any

import psycopg2
import psycopg2.extras
import psycopg2.pool

logger = logging.getLogger(__name__)

_pool: psycopg2.pool.SimpleConnectionPool | None = None


def init_pool(database_url: str) -> None:
    global _pool
    _pool = psycopg2.pool.SimpleConnectionPool(1, 4, database_url)
    logger.info("数据库连接池初始化完成")


@contextmanager
def get_conn():
    assert _pool is not None, "请先调用 init_pool()"
    conn = _pool.getconn()
    try:
        yield conn
        conn.commit()
    except Exception:
        conn.rollback()
        raise
    finally:
        _pool.putconn(conn)


# ─── 仓库注册 ─────────────────────────────────────────────────────────────────

def ensure_repo(conn, name: str, url: str) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO repos (name, url)
            VALUES (%s, %s)
            ON CONFLICT (name) DO NOTHING
            """,
            (name, url),
        )


# ─── Collector 状态 ───────────────────────────────────────────────────────────

def get_last_sha(conn, repo_name: str) -> str | None:
    with conn.cursor() as cur:
        cur.execute(
            "SELECT last_commit_sha FROM collector_state WHERE repo_name = %s",
            (repo_name,),
        )
        row = cur.fetchone()
        return row[0] if row else None


def update_state(conn, repo_name: str, last_sha: str,
                 processed_count: int, error: str | None = None) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO collector_state
                (repo_name, last_collected_at, last_commit_sha,
                 total_commits_processed, last_error, last_error_at)
            VALUES (%s, NOW(), %s, %s, %s, CASE WHEN %s IS NOT NULL THEN NOW() END)
            ON CONFLICT (repo_name) DO UPDATE SET
                last_collected_at       = NOW(),
                last_commit_sha         = EXCLUDED.last_commit_sha,
                total_commits_processed = collector_state.total_commits_processed
                                          + EXCLUDED.total_commits_processed,
                last_error              = EXCLUDED.last_error,
                last_error_at           = EXCLUDED.last_error_at
            """,
            (repo_name, last_sha, processed_count, error, error),
        )


def set_error(conn, repo_name: str, error: str) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO collector_state (repo_name, last_error, last_error_at)
            VALUES (%s, %s, NOW())
            ON CONFLICT (repo_name) DO UPDATE SET
                last_error    = EXCLUDED.last_error,
                last_error_at = NOW()
            """,
            (repo_name, error),
        )


# ─── Commit 写入 ──────────────────────────────────────────────────────────────

def upsert_commit(conn, row: dict) -> int | None:
    """插入 commit_metrics 行，返回 id（已存在则返回 None）"""
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO commit_metrics (
                repo_name, commit_sha, commit_author_email, commit_author_name,
                committed_at, schema_version, git_ai_version,
                prompt_count, total_ai_additions, total_ai_deletions,
                accepted_lines, overridden_lines, tools_used, models_used
            ) VALUES (
                %(repo_name)s, %(commit_sha)s, %(author_email)s, %(author_name)s,
                %(committed_at)s, %(schema_version)s, %(git_ai_version)s,
                %(prompt_count)s, %(total_ai_additions)s, %(total_ai_deletions)s,
                %(accepted_lines)s, %(overridden_lines)s,
                %(tools_used)s::jsonb, %(models_used)s::jsonb
            )
            ON CONFLICT (repo_name, commit_sha) DO NOTHING
            RETURNING id
            """,
            {
                **row,
                "tools_used": json.dumps(row.get("tools_used", [])),
                "models_used": json.dumps(row.get("models_used", [])),
            },
        )
        result = cur.fetchone()
        return result[0] if result else None


def upsert_prompts(conn, rows: list[dict]) -> None:
    if not rows:
        return
    with conn.cursor() as cur:
        psycopg2.extras.execute_values(
            cur,
            """
            INSERT INTO prompt_metrics (
                repo_name, commit_sha, prompt_hash, tool, model,
                human_author_email, total_additions, total_deletions,
                accepted_lines, overridden_lines
            ) VALUES %s
            ON CONFLICT (repo_name, commit_sha, prompt_hash) DO NOTHING
            """,
            [
                (
                    r["repo_name"], r["commit_sha"], r["prompt_hash"],
                    r["tool"], r["model"], r["human_author_email"],
                    r["total_additions"], r["total_deletions"],
                    r["accepted_lines"], r["overridden_lines"],
                )
                for r in rows
            ],
        )


def upsert_files(conn, rows: list[dict]) -> None:
    if not rows:
        return
    with conn.cursor() as cur:
        psycopg2.extras.execute_values(
            cur,
            """
            INSERT INTO file_metrics (
                repo_name, commit_sha, file_path, attributed_lines, prompt_hashes
            ) VALUES %s
            ON CONFLICT (repo_name, commit_sha, file_path) DO NOTHING
            """,
            [
                (
                    r["repo_name"], r["commit_sha"], r["file_path"],
                    r["attributed_lines"],
                    json.dumps(r.get("prompt_hashes", [])),
                )
                for r in rows
            ],
        )
