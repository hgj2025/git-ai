package main

import (
	"context"
	"embed"
	"flag"
	"fmt"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"
)

//go:embed static
var staticFiles embed.FS

func main() {
	port := flag.Int("port", 8080, "HTTP port")
	dbPath := flag.String("db", "metrics.db", "SQLite database path")
	token := flag.String("token", "", "Bearer token for /api/report (empty = no auth)")
	flag.Parse()

	// Allow env var overrides
	if v := os.Getenv("GIT_AI_SERVER_PORT"); v != "" {
		fmt.Sscanf(v, "%d", port)
	}
	if v := os.Getenv("GIT_AI_SERVER_DB"); v != "" {
		*dbPath = v
	}
	if v := os.Getenv("GIT_AI_SERVER_TOKEN"); v != "" {
		*token = v
	}

	db, err := initDB(*dbPath)
	if err != nil {
		log.Fatalf("init db: %v", err)
	}
	defer db.Close()

	mux := newMux(db, *token)

	srv := &http.Server{
		Addr:         fmt.Sprintf(":%d", *port),
		Handler:      mux,
		ReadTimeout:  15 * time.Second,
		WriteTimeout: 30 * time.Second,
	}

	go func() {
		log.Printf("git-ai metrics server listening on http://localhost:%d", *port)
		if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatalf("listen: %v", err)
		}
	}()

	quit := make(chan os.Signal, 1)
	signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)
	<-quit

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	srv.Shutdown(ctx) //nolint:errcheck
}
