"""
配置加载：从 repos.yaml + 环境变量读取所有配置
"""

import os
from dataclasses import dataclass, field
from typing import Optional
import yaml


@dataclass
class RepoConfig:
    name: str
    url: str
    auth: str = "ssh"                   # ssh | token
    ssh_key: str = "/root/.ssh/id_rsa"
    token_env: str = ""                  # 存放 token 的环境变量名
    default_branch: str = "main"

    @property
    def token(self) -> str:
        """从环境变量读取 token 值"""
        if not self.token_env:
            return ""
        return os.environ.get(self.token_env, "")

    @property
    def clone_url(self) -> str:
        """注入 token 后的克隆 URL"""
        if self.auth == "token" and self.token:
            # https://token@github.com/org/repo.git
            url = self.url
            if url.startswith("https://"):
                url = url.replace("https://", f"https://oauth2:{self.token}@", 1)
            return url
        return self.url


@dataclass
class CollectorConfig:
    repos: list[RepoConfig] = field(default_factory=list)
    database_url: str = ""
    collect_interval: int = 300
    repos_dir: str = "/data/repos"


def load_config(repos_yaml_path: str = "/app/config/repos.yaml") -> CollectorConfig:
    cfg = CollectorConfig(
        database_url=os.environ.get("DATABASE_URL", ""),
        collect_interval=int(os.environ.get("COLLECT_INTERVAL_SECONDS", "300")),
        repos_dir=os.environ.get("REPOS_DIR", "/data/repos"),
    )

    with open(repos_yaml_path) as f:
        data = yaml.safe_load(f)

    for r in data.get("repos", []):
        cfg.repos.append(RepoConfig(
            name=r["name"],
            url=r["url"],
            auth=r.get("auth", "ssh"),
            ssh_key=r.get("ssh_key", "/root/.ssh/id_rsa"),
            token_env=r.get("token_env", ""),
            default_branch=r.get("default_branch", "main"),
        ))

    return cfg
