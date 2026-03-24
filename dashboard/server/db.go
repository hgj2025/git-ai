package main

import (
	"database/sql"
	"fmt"
	"time"

	_ "modernc.org/sqlite"
)

func initDB(path string) (*sql.DB, error) {
	db, err := sql.Open("sqlite", path+"?_journal=WAL&_timeout=5000")
	if err != nil {
		return nil, fmt.Errorf("open db: %w", err)
	}
	db.SetMaxOpenConns(1)
	if err := migrate(db); err != nil {
		return nil, fmt.Errorf("migrate: %w", err)
	}
	return db, nil
}

func migrate(db *sql.DB) error {
	_, err := db.Exec(`
CREATE TABLE IF NOT EXISTS commit_metrics (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    repo             TEXT NOT NULL,
    commit_sha       TEXT NOT NULL,
    author_email     TEXT NOT NULL DEFAULT '',
    author_name      TEXT NOT NULL DEFAULT '',
    committed_at     DATETIME,
    tool             TEXT NOT NULL DEFAULT '',
    model            TEXT NOT NULL DEFAULT '',
    total_additions  INTEGER NOT NULL DEFAULT 0,
    accepted_lines   INTEGER NOT NULL DEFAULT 0,
    overridden_lines INTEGER NOT NULL DEFAULT 0,
    reported_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(repo, commit_sha, tool)
);
CREATE INDEX IF NOT EXISTS idx_committed_at ON commit_metrics(committed_at);
CREATE INDEX IF NOT EXISTS idx_repo          ON commit_metrics(repo);
CREATE INDEX IF NOT EXISTS idx_author_email  ON commit_metrics(author_email);
`)
	return err
}

type promptMetric struct {
	Tool            string `json:"tool"`
	Model           string `json:"model"`
	TotalAdditions  int    `json:"total_additions"`
	AcceptedLines   int    `json:"accepted_lines"`
	OverriddenLines int    `json:"overridden_lines"`
}

type commitPayload struct {
	CommitSHA   string         `json:"commit_sha"`
	AuthorEmail string         `json:"author_email"`
	AuthorName  string         `json:"author_name"`
	CommittedAt time.Time      `json:"committed_at"`
	Prompts     []promptMetric `json:"prompts"`
}

type reportRequest struct {
	Repo    string          `json:"repo"`
	Commits []commitPayload `json:"commits"`
}

func insertReport(db *sql.DB, req reportRequest) (int, error) {
	inserted := 0
	tx, err := db.Begin()
	if err != nil {
		return 0, err
	}
	defer tx.Rollback() //nolint:errcheck

	stmt, err := tx.Prepare(`
INSERT OR IGNORE INTO commit_metrics
    (repo, commit_sha, author_email, author_name, committed_at,
     tool, model, total_additions, accepted_lines, overridden_lines)
VALUES (?,?,?,?,?,?,?,?,?,?)
`)
	if err != nil {
		return 0, err
	}
	defer stmt.Close()

	for _, c := range req.Commits {
		// Store in SQLite-compatible format (no timezone suffix)
		committedAt := c.CommittedAt.UTC().Format("2006-01-02 15:04:05")
		for _, p := range c.Prompts {
			res, err := stmt.Exec(
				req.Repo, c.CommitSHA, c.AuthorEmail, c.AuthorName, committedAt,
				p.Tool, p.Model, p.TotalAdditions, p.AcceptedLines, p.OverriddenLines,
			)
			if err != nil {
				return inserted, err
			}
			n, _ := res.RowsAffected()
			inserted += int(n)
		}
	}
	return inserted, tx.Commit()
}

// ── query types ──────────────────────────────────────────────────────────────

type summaryRow struct {
	TotalAILines   int64   `json:"total_ai_lines"`
	AcceptedLines  int64   `json:"accepted_lines"`
	AcceptanceRate float64 `json:"acceptance_rate"`
	TotalCommits   int64   `json:"total_commits"`
	ActiveDevs     int64   `json:"active_devs"`
}

type dailyRow struct {
	Date          string `json:"date"`
	AILines       int64  `json:"ai_lines"`
	AcceptedLines int64  `json:"accepted_lines"`
}

type toolRow struct {
	Tool          string  `json:"tool"`
	Commits       int64   `json:"commits"`
	AILines       int64   `json:"ai_lines"`
	AcceptedLines int64   `json:"accepted_lines"`
	AcceptanceRate float64 `json:"acceptance_rate"`
}

type modelRow struct {
	Model          string  `json:"model"`
	Commits        int64   `json:"commits"`
	AILines        int64   `json:"ai_lines"`
	AcceptanceRate float64 `json:"acceptance_rate"`
}

type devRow struct {
	Email          string  `json:"email"`
	Name           string  `json:"name"`
	Commits        int64   `json:"commits"`
	AILines        int64   `json:"ai_lines"`
	AcceptedLines  int64   `json:"accepted_lines"`
	AcceptanceRate float64 `json:"acceptance_rate"`
}

type repoRow struct {
	Repo           string  `json:"repo"`
	Commits        int64   `json:"commits"`
	AILines        int64   `json:"ai_lines"`
	AcceptedLines  int64   `json:"accepted_lines"`
	AcceptanceRate float64 `json:"acceptance_rate"`
}

type statsResponse struct {
	Summary    summaryRow  `json:"summary"`
	Daily      []dailyRow  `json:"daily"`
	Tools      []toolRow   `json:"tools"`
	Models     []modelRow  `json:"models"`
	Developers []devRow    `json:"developers"`
	Repos      []repoRow   `json:"repos"`
}

func queryStats(db *sql.DB, days int, repo string) (statsResponse, error) {
	var res statsResponse

	repoFilter := ""
	args := []any{days}
	if repo != "" && repo != "all" {
		repoFilter = "AND repo = ?"
		args = append(args, repo)
	}

	// summary
	row := db.QueryRow(fmt.Sprintf(`
SELECT
    COALESCE(SUM(total_additions),0),
    COALESCE(SUM(accepted_lines),0),
    CASE WHEN SUM(total_additions)>0 THEN CAST(SUM(accepted_lines) AS REAL)/SUM(total_additions) ELSE 0 END,
    COUNT(DISTINCT commit_sha),
    COUNT(DISTINCT author_email)
FROM commit_metrics
WHERE committed_at >= datetime('now','-%d days') %s
`, days, repoFilter), args[1:]...)
	if err := row.Scan(
		&res.Summary.TotalAILines, &res.Summary.AcceptedLines,
		&res.Summary.AcceptanceRate, &res.Summary.TotalCommits, &res.Summary.ActiveDevs,
	); err != nil {
		return res, err
	}

	// daily (last N days)
	dailyArgs := args
	rows, err := db.Query(fmt.Sprintf(`
SELECT
    date(committed_at) as d,
    COALESCE(SUM(total_additions),0),
    COALESCE(SUM(accepted_lines),0)
FROM commit_metrics
WHERE committed_at >= datetime('now','-%d days') %s
GROUP BY d ORDER BY d
`, days, repoFilter), dailyArgs[1:]...)
	if err != nil {
		return res, err
	}
	defer rows.Close()
	for rows.Next() {
		var r dailyRow
		if err := rows.Scan(&r.Date, &r.AILines, &r.AcceptedLines); err != nil {
			return res, err
		}
		res.Daily = append(res.Daily, r)
	}

	// tools
	rows2, err := db.Query(fmt.Sprintf(`
SELECT
    tool,
    COUNT(DISTINCT commit_sha),
    COALESCE(SUM(total_additions),0),
    COALESCE(SUM(accepted_lines),0),
    CASE WHEN SUM(total_additions)>0 THEN CAST(SUM(accepted_lines) AS REAL)/SUM(total_additions) ELSE 0 END
FROM commit_metrics
WHERE committed_at >= datetime('now','-%d days') AND tool!='' %s
GROUP BY tool ORDER BY SUM(total_additions) DESC
`, days, repoFilter), dailyArgs[1:]...)
	if err != nil {
		return res, err
	}
	defer rows2.Close()
	for rows2.Next() {
		var r toolRow
		if err := rows2.Scan(&r.Tool, &r.Commits, &r.AILines, &r.AcceptedLines, &r.AcceptanceRate); err != nil {
			return res, err
		}
		res.Tools = append(res.Tools, r)
	}

	// models
	rows3, err := db.Query(fmt.Sprintf(`
SELECT
    model,
    COUNT(DISTINCT commit_sha),
    COALESCE(SUM(total_additions),0),
    CASE WHEN SUM(total_additions)>0 THEN CAST(SUM(accepted_lines) AS REAL)/SUM(total_additions) ELSE 0 END
FROM commit_metrics
WHERE committed_at >= datetime('now','-%d days') AND model!='' %s
GROUP BY model ORDER BY SUM(total_additions) DESC LIMIT 20
`, days, repoFilter), dailyArgs[1:]...)
	if err != nil {
		return res, err
	}
	defer rows3.Close()
	for rows3.Next() {
		var r modelRow
		if err := rows3.Scan(&r.Model, &r.Commits, &r.AILines, &r.AcceptanceRate); err != nil {
			return res, err
		}
		res.Models = append(res.Models, r)
	}

	// developers
	rows4, err := db.Query(fmt.Sprintf(`
SELECT
    author_email,
    MAX(author_name),
    COUNT(DISTINCT commit_sha),
    COALESCE(SUM(total_additions),0),
    COALESCE(SUM(accepted_lines),0),
    CASE WHEN SUM(total_additions)>0 THEN CAST(SUM(accepted_lines) AS REAL)/SUM(total_additions) ELSE 0 END
FROM commit_metrics
WHERE committed_at >= datetime('now','-%d days') AND author_email!='' %s
GROUP BY author_email ORDER BY SUM(total_additions) DESC LIMIT 50
`, days, repoFilter), dailyArgs[1:]...)
	if err != nil {
		return res, err
	}
	defer rows4.Close()
	for rows4.Next() {
		var r devRow
		if err := rows4.Scan(&r.Email, &r.Name, &r.Commits, &r.AILines, &r.AcceptedLines, &r.AcceptanceRate); err != nil {
			return res, err
		}
		res.Developers = append(res.Developers, r)
	}

	// repos
	rows5, err := db.Query(fmt.Sprintf(`
SELECT
    repo,
    COUNT(DISTINCT commit_sha),
    COALESCE(SUM(total_additions),0),
    COALESCE(SUM(accepted_lines),0),
    CASE WHEN SUM(total_additions)>0 THEN CAST(SUM(accepted_lines) AS REAL)/SUM(total_additions) ELSE 0 END
FROM commit_metrics
WHERE committed_at >= datetime('now','-%d days') %s
GROUP BY repo ORDER BY SUM(total_additions) DESC
`, days, repoFilter), dailyArgs[1:]...)
	if err != nil {
		return res, err
	}
	defer rows5.Close()
	for rows5.Next() {
		var r repoRow
		if err := rows5.Scan(&r.Repo, &r.Commits, &r.AILines, &r.AcceptedLines, &r.AcceptanceRate); err != nil {
			return res, err
		}
		res.Repos = append(res.Repos, r)
	}

	return res, nil
}
