package main

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"io/fs"
	"net/http"
	"strconv"
)

func newMux(db *sql.DB, token string) *http.ServeMux {
	mux := http.NewServeMux()
	mux.HandleFunc("POST /api/report", authMiddleware(token, handleReport(db)))
	mux.HandleFunc("GET /api/stats", handleStats(db))
	mux.HandleFunc("GET /api/repos", handleRepos(db))
	mux.HandleFunc("GET /install.sh", handleInstallScript(token))
	mux.HandleFunc("GET /uninstall.sh", handleUninstallScript)

	// Serve static/ directory at root so / serves index.html directly
	sub, _ := fs.Sub(staticFiles, "static")
	mux.Handle("/", http.FileServer(http.FS(sub)))
	return mux
}

func authMiddleware(token string, next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if token == "" {
			next(w, r)
			return
		}
		auth := r.Header.Get("Authorization")
		if auth != "Bearer "+token {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		next(w, r)
	}
}

func handleReport(db *sql.DB) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		var req reportRequest
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, "invalid json: "+err.Error(), http.StatusBadRequest)
			return
		}
		if req.Repo == "" {
			http.Error(w, "repo is required", http.StatusBadRequest)
			return
		}
		inserted, err := insertReport(db, req)
		if err != nil {
			http.Error(w, "db error: "+err.Error(), http.StatusInternalServerError)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{"ok": true, "inserted": inserted}) //nolint:errcheck
	}
}

func handleStats(db *sql.DB) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		days := 30
		if d := r.URL.Query().Get("days"); d != "" {
			if n, err := strconv.Atoi(d); err == nil && n > 0 {
				days = n
			}
		}
		repo := r.URL.Query().Get("repo")

		stats, err := queryStats(db, days, repo)
		if err != nil {
			http.Error(w, "query error: "+err.Error(), http.StatusInternalServerError)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(stats) //nolint:errcheck
	}
}

func serverBaseURL(r *http.Request) string {
	scheme := "http"
	if r.TLS != nil || r.Header.Get("X-Forwarded-Proto") == "https" {
		scheme = "https"
	}
	return scheme + "://" + r.Host
}

func handleInstallScript(token string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		baseURL := serverBaseURL(r)

		// Read embedded setup.sh and inject server config at the top
		setupBytes, err := staticFiles.ReadFile("static/setup.sh")
		if err != nil {
			http.Error(w, "setup script not found", http.StatusInternalServerError)
			return
		}

		// Inject server/token as defaults before the script runs
		inject := fmt.Sprintf(
			"# --- injected by git-ai-server ---\n"+
				"export GIT_AI_METRICS_SERVER=\"%s\"\n",
			baseURL,
		)
		if token != "" {
			inject += fmt.Sprintf("export GIT_AI_METRICS_TOKEN=\"%s\"\n", token)
		}
		inject += "# -----------------------------------\n"

		// Insert after the shebang line
		setup := string(setupBytes)
		shebangEnd := 0
		if len(setup) > 0 && setup[0] == '#' {
			if nl := indexOf(setup, '\n'); nl >= 0 {
				shebangEnd = nl + 1
			}
		}
		script := setup[:shebangEnd] + inject + setup[shebangEnd:]

		w.Header().Set("Content-Type", "text/plain; charset=utf-8")
		fmt.Fprint(w, script)
	}
}

func indexOf(s string, b byte) int {
	for i := 0; i < len(s); i++ {
		if s[i] == b {
			return i
		}
	}
	return -1
}

var uninstallScript = `#!/usr/bin/env bash
# git-ai 团队 AI 看板 — 开发者一键卸载
# 用法: curl -fsSL http://your-server:8080/uninstall.sh | bash
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BOLD='\033[1m'; NC='\033[0m'
info()    { echo -e "\033[0;34m[info]\033[0m $*"; }
success() { echo -e "${GREEN}[ok]${NC} $*"; }
warn()    { echo -e "${YELLOW}[warn]${NC} $*"; }
step()    { echo -e "\n${BOLD}▶ $*${NC}"; }

step "清除团队统计服务配置"

if git config --global --get git-ai.metrics-server &>/dev/null; then
    git config --global --unset git-ai.metrics-server
    success "已清除 git-ai.metrics-server"
else
    info "git-ai.metrics-server 未配置，跳过"
fi

if git config --global --get git-ai.metrics-token &>/dev/null; then
    git config --global --unset git-ai.metrics-token
    success "已清除 git-ai.metrics-token"
fi

step "从所有仓库移除 post-push hook"

SEARCH_DIRS=("$HOME/Code" "$HOME/Projects" "$HOME/workspace" "$HOME/src" "$HOME/dev")
REMOVED=0

for d in "${SEARCH_DIRS[@]}"; do
    [ -d "$d" ] || continue
    while IFS= read -r git_dir; do
        repo=$(dirname "$git_dir")
        # 检查 managed hooks 目录下的 post-push
        hooks_dir=$(git -C "$repo" config --local core.hooksPath 2>/dev/null || true)
        hook_file="${hooks_dir:-$git_dir/hooks}/post-push"
        if [ -f "$hook_file" ] && grep -q "git-ai post-push metrics" "$hook_file" 2>/dev/null; then
            rm -f "$hook_file"
            ((REMOVED++)) || true
            echo "  移除: $hook_file"
        fi
    done < <(find "$d" -maxdepth 4 -name ".git" -type d 2>/dev/null)
done

success "已从 $REMOVED 个仓库移除 post-push hook"

echo ""
echo -e "${GREEN}${BOLD}✅ 卸载完成${NC}"
echo ""
echo "如需彻底卸载 git-ai hooks，在各仓库目录执行："
echo "  git-ai uninstall-hooks"
echo ""
`

func handleUninstallScript(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	fmt.Fprint(w, uninstallScript)
}

func handleRepos(db *sql.DB) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		rows, err := db.Query(`SELECT DISTINCT repo FROM commit_metrics ORDER BY repo`)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		defer rows.Close()
		var repos []string
		for rows.Next() {
			var repo string
			if err := rows.Scan(&repo); err == nil {
				repos = append(repos, repo)
			}
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(repos) //nolint:errcheck
	}
}
