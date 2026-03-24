"""
仓库生命周期管理：克隆、增量 fetch、git notes 拉取。
"""

import logging
import os
import subprocess
from pathlib import Path

from config import RepoConfig

logger = logging.getLogger(__name__)


def _run(cmd: list[str], cwd: str | None = None,
         env_extra: dict | None = None) -> subprocess.CompletedProcess:
    env = os.environ.copy()
    if env_extra:
        env.update(env_extra)
    result = subprocess.run(
        cmd, cwd=cwd, capture_output=True, text=True, env=env
    )
    return result


def _ssh_env(key_path: str) -> dict:
    return {
        "GIT_SSH_COMMAND": (
            f"ssh -i {key_path} "
            "-o StrictHostKeyChecking=accept-new "
            "-o BatchMode=yes "
            "-o ConnectTimeout=10"
        )
    }


def repo_path(repos_dir: str, repo_name: str) -> Path:
    return Path(repos_dir) / f"{repo_name}.git"


def ensure_repo(cfg: RepoConfig, repos_dir: str) -> Path:
    """
    确保本地有该仓库的 bare clone。
    首次运行克隆，后续 fetch。
    返回本地仓库路径。
    """
    local = repo_path(repos_dir, cfg.name)
    env = _ssh_env(cfg.ssh_key) if cfg.auth == "ssh" else {}

    if not local.exists():
        logger.info(f"[{cfg.name}] 首次克隆 {cfg.url}")
        os.makedirs(str(local.parent), exist_ok=True)
        r = _run(
            ["git", "clone", "--bare", "--filter=blob:none", cfg.clone_url, str(local)],
            env_extra=env,
        )
        if r.returncode != 0:
            raise RuntimeError(f"[{cfg.name}] 克隆失败: {r.stderr.strip()}")
        logger.info(f"[{cfg.name}] 克隆完成")
    else:
        logger.debug(f"[{cfg.name}] fetch 增量更新")
        r = _run(
            ["git", "fetch", "--prune", "origin",
             f"+refs/heads/{cfg.default_branch}:refs/heads/{cfg.default_branch}"],
            cwd=str(local), env_extra=env,
        )
        if r.returncode != 0:
            logger.warning(f"[{cfg.name}] fetch heads 失败: {r.stderr.strip()}")

    # 单独 fetch git notes（git 默认不拉取 notes）
    r = _run(
        ["git", "fetch", "origin", "refs/notes/ai:refs/notes/ai"],
        cwd=str(local), env_extra=env,
    )
    if r.returncode != 0:
        # notes ref 不存在时会失败，属于正常情况（该仓库还没有 AI 数据）
        logger.debug(f"[{cfg.name}] 无 git notes 或 fetch 失败: {r.stderr.strip()}")

    return local


def list_noted_commits(repo_local: Path) -> list[str]:
    """
    返回所有附有 git-ai notes 的 commit SHA 列表。
    `git notes --ref=ai list` 输出格式：<note-blob-sha> <commit-sha>
    """
    r = _run(["git", "notes", "--ref=ai", "list"], cwd=str(repo_local))
    if r.returncode != 0:
        return []
    shas = []
    for line in r.stdout.strip().split("\n"):
        parts = line.split()
        if len(parts) == 2:
            shas.append(parts[1])   # 取第二列：被标注的 commit SHA
    return shas


def new_commits_since(repo_local: Path, last_sha: str | None) -> list[str]:
    """
    返回 last_sha 之后的所有 commit SHA（从旧到新）。
    若 last_sha 为 None，返回所有 commit。
    """
    if last_sha:
        # 验证 last_sha 是否仍存在（防止 force-push 后悬空）
        r = _run(["git", "cat-file", "-t", last_sha], cwd=str(repo_local))
        if r.returncode != 0 or r.stdout.strip() != "commit":
            logger.warning(f"last_sha {last_sha} 已不存在（可能被 force-push），重置增量状态")
            last_sha = None

    cmd = ["git", "rev-list", "--reverse", "HEAD"]
    if last_sha:
        cmd = ["git", "rev-list", "--reverse", f"{last_sha}..HEAD"]

    r = _run(cmd, cwd=str(repo_local))
    if r.returncode != 0:
        return []
    return [s for s in r.stdout.strip().split("\n") if s]


def read_note(repo_local: Path, commit_sha: str) -> str | None:
    """读取某个 commit 的 git note 内容"""
    r = _run(
        ["git", "notes", "--ref=ai", "show", commit_sha],
        cwd=str(repo_local),
    )
    if r.returncode != 0:
        return None
    return r.stdout


def read_commit_metadata(repo_local: Path, commit_sha: str) -> dict:
    """读取 commit 的作者、时间、消息"""
    r = _run(
        ["git", "show", "-s",
         "--format=%ae\t%an\t%aI\t%s",
         commit_sha],
        cwd=str(repo_local),
    )
    if r.returncode != 0:
        return {}
    parts = r.stdout.strip().split("\t", 3)
    if len(parts) < 4:
        return {}
    return {
        "author_email": parts[0],
        "author_name":  parts[1],
        "committed_at": parts[2],
        "message":      parts[3],
    }
