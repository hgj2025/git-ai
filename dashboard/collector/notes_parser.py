"""
解析 git-ai authorship log 格式（authorship/3.0.0）。

格式规范见：specs/git_ai_standard_v3.0.0.md

结构：
    <attestation-section>
    ---
    <json-metadata>

attestation-section：
    src/main.rs
      abcd1234abcd1234 1-10,15-20
      efgh5678efgh5678 25

json-metadata：
    {
      "schema_version": "authorship/3.0.0",
      "prompts": {
        "abcd1234abcd1234": {
          "agent_id": {"tool": "cursor", "model": "claude-sonnet-4-5", "id": "..."},
          "human_author": "dev@example.com",
          "total_additions": 25,
          "total_deletions": 5,
          "accepted_lines": 20,
          "overriden_lines": 5      <- 注意：规范中有拼写错误
        }
      }
    }
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Optional


# ─── 数据结构 ────────────────────────────────────────────────────────────────

@dataclass
class AttestationEntry:
    hash: str                            # 可能是 7 位或 16 位
    line_count: int                      # attestation 中归属的行数


@dataclass
class FileAttestation:
    file_path: str
    entries: list[AttestationEntry] = field(default_factory=list)

    @property
    def total_lines(self) -> int:
        return sum(e.line_count for e in self.entries)


@dataclass
class PromptRecord:
    hash: str                            # 16 位 session hash
    tool: str
    model: str
    human_author: str
    total_additions: int
    total_deletions: int
    accepted_lines: int
    overridden_lines: int
    committed_lines: int = 0             # 从 attestation 推算，见 ParsedNote.resolve()


@dataclass
class ParsedNote:
    schema_version: str
    git_ai_version: str
    base_commit_sha: str
    attestations: list[FileAttestation]
    prompts: list[PromptRecord]

    @property
    def total_committed_ai_lines(self) -> int:
        return sum(f.total_lines for f in self.attestations)


# ─── 工具函数 ─────────────────────────────────────────────────────────────────

def _count_lines(ranges_str: str) -> int:
    """将 '1-10,15,20-25' 转换为总行数"""
    if not ranges_str.strip():
        return 0
    total = 0
    for part in ranges_str.split(","):
        part = part.strip()
        if not part:
            continue
        if "-" in part:
            try:
                s, e = part.split("-", 1)
                total += max(0, int(e) - int(s) + 1)
            except ValueError:
                pass
        elif part.isdigit():
            total += 1
    return total


def _match_hash(short: str, full_keys: list[str]) -> Optional[str]:
    """用短 hash（7 位+）匹配 16 位完整 hash"""
    for key in full_keys:
        if key.startswith(short) or short.startswith(key):
            return key
    return None


# ─── 解析入口 ─────────────────────────────────────────────────────────────────

def parse_note(text: str) -> Optional[ParsedNote]:
    """
    解析一条 git note 的文本内容。
    返回 None 表示该 note 不是有效的 git-ai authorship log。
    """
    divider = "\n---\n"
    if divider not in text:
        return None

    attestation_text, metadata_json = text.split(divider, 1)

    # ── 解析 JSON metadata ────────────────────────────────────────────────────
    try:
        meta = json.loads(metadata_json.strip())
    except (json.JSONDecodeError, ValueError):
        return None

    raw_prompts: dict = meta.get("prompts", {})
    if not raw_prompts:
        return None

    # ── 解析 attestation section ──────────────────────────────────────────────
    attestations: list[FileAttestation] = []
    current_file: Optional[FileAttestation] = None

    for line in attestation_text.split("\n"):
        if not line:
            continue
        if line[:2] == "  ":
            # 条目行：  <hash> <ranges>
            parts = line.strip().split(" ", 1)
            if not parts:
                continue
            h = parts[0]
            ranges = parts[1] if len(parts) > 1 else ""
            if current_file is not None:
                current_file.entries.append(AttestationEntry(
                    hash=h,
                    line_count=_count_lines(ranges),
                ))
        else:
            # 文件路径行
            path = line.strip().strip('"')
            if path:
                current_file = FileAttestation(file_path=path)
                attestations.append(current_file)

    # ── 从 attestation 推算每个 session 的实际 commit 行数 ────────────────────
    session_committed: dict[str, int] = {}
    all_hashes = list(raw_prompts.keys())
    for fa in attestations:
        for entry in fa.entries:
            full = _match_hash(entry.hash, all_hashes)
            if full:
                session_committed[full] = session_committed.get(full, 0) + entry.line_count

    # ── 构建 PromptRecord 列表 ────────────────────────────────────────────────
    prompts: list[PromptRecord] = []
    for ph, p in raw_prompts.items():
        agent_id = p.get("agent_id", {})
        prompts.append(PromptRecord(
            hash=ph,
            tool=agent_id.get("tool", "unknown"),
            model=agent_id.get("model", "unknown"),
            human_author=p.get("human_author", ""),
            total_additions=p.get("total_additions", 0),
            total_deletions=p.get("total_deletions", 0),
            accepted_lines=p.get("accepted_lines", 0),
            overridden_lines=p.get("overriden_lines", 0),  # 规范拼写错误，保持兼容
            committed_lines=session_committed.get(ph, 0),
        ))

    return ParsedNote(
        schema_version=meta.get("schema_version", ""),
        git_ai_version=meta.get("git_ai_version", ""),
        base_commit_sha=meta.get("base_commit_sha", ""),
        attestations=attestations,
        prompts=prompts,
    )
