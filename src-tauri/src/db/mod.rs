pub mod providers;

use rusqlite::Connection;
use std::path::PathBuf;

/// 数据库路径：跟随 Tauri app data 目录
pub fn db_path(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("agentbox.db")
}

/// 初始化数据库连接并创建表结构
pub fn init_db(path: &PathBuf) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    init_db_with_conn(&conn);
    Ok(conn)
}

/// 在已有连接上执行建表和迁移（测试可复用）
pub fn init_db_with_conn(conn: &Connection) {
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .expect("pragma failed");
    create_tables(conn).expect("create tables failed");
    run_migrations(conn).expect("migrations failed");
}

fn create_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY
        );

        CREATE TABLE IF NOT EXISTS providers (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL UNIQUE,
            base_url    TEXT NOT NULL,
            api_key_env TEXT NOT NULL,
            proxy_mode  TEXT NOT NULL DEFAULT 'direct' CHECK(proxy_mode IN ('system', 'custom', 'direct')),
            proxy_url   TEXT,
            auth_header TEXT,
            billing_type TEXT NOT NULL DEFAULT 'pay_per_use' CHECK(billing_type IN ('plan', 'pay_per_use')),
            is_active   BOOLEAN DEFAULT 1,
            created_at  TEXT DEFAULT (datetime('now')),
            updated_at  TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS models (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            provider_id  INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
            model_id     TEXT NOT NULL,
            display_name TEXT,
            auth_type    TEXT NOT NULL,
            is_default   BOOLEAN DEFAULT 0,
            config_json  TEXT,
            last_success_at TEXT,
            created_at   TEXT DEFAULT (datetime('now')),
            UNIQUE(provider_id, model_id)
        );

        CREATE TABLE IF NOT EXISTS request_logs (
            id                   INTEGER PRIMARY KEY AUTOINCREMENT,
            request_id           TEXT UNIQUE NOT NULL,
            session_id           TEXT,
            timestamp            TEXT DEFAULT (datetime('now')),
            provider_id          INTEGER REFERENCES providers(id),
            model_id             TEXT,
            auth_type            TEXT,
            endpoint             TEXT,
            input_tokens         INTEGER DEFAULT 0,
            output_tokens        INTEGER DEFAULT 0,
            cache_read_tokens    INTEGER DEFAULT 0,
            cache_write_tokens   INTEGER DEFAULT 0,
            reasoning_tokens     INTEGER DEFAULT 0,
            duration_ms          INTEGER,
            time_to_first_ms     INTEGER,
            status_code          INTEGER,
            is_stream            BOOLEAN DEFAULT 0,
            context_compressed   BOOLEAN DEFAULT 0,
            original_tokens      INTEGER DEFAULT 0,
            tokens_saved         INTEGER DEFAULT 0,
            error_message        TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_request_logs_session ON request_logs(session_id);
        CREATE INDEX IF NOT EXISTS idx_request_logs_timestamp ON request_logs(timestamp);
        CREATE INDEX IF NOT EXISTS idx_request_logs_provider ON request_logs(provider_id, model_id);

        CREATE VIEW IF NOT EXISTS cost_daily AS
        SELECT
            date(timestamp)     AS date,
            provider_id,
            model_id,
            COUNT(*)            AS request_count,
            SUM(input_tokens)   AS total_input,
            SUM(output_tokens)  AS total_output,
            SUM(cache_read_tokens)  AS total_cache_read,
            SUM(reasoning_tokens)   AS total_reasoning,
            SUM(duration_ms)        AS total_duration_ms,
            SUM(CASE WHEN error_message IS NOT NULL THEN 1 ELSE 0 END) AS error_count,
            SUM(tokens_saved)       AS total_tokens_saved
        FROM request_logs
        GROUP BY date, provider_id, model_id;",
    )
    .map_err(|e| e.to_string())
}

fn run_migrations(conn: &Connection) -> Result<(), String> {
    let current: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if current < 1 {
        conn.execute("INSERT OR IGNORE INTO schema_version (version) VALUES (1)", [])
            .map_err(|e| e.to_string())?;
    }

    if current < 2 {
        // 早期版本的 providers 表可能缺少 auth_header 列
        // SQLite 不支持 ADD COLUMN IF NOT EXISTS，用 PRAGMA 检查
        let has_auth_header: bool = conn
            .prepare("PRAGMA table_info(providers)")
            .map_err(|e| e.to_string())?
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .any(|col| col == "auth_header");

        if !has_auth_header {
            conn.execute("ALTER TABLE providers ADD COLUMN auth_header TEXT", [])
                .map_err(|e| e.to_string())?;
        }

        conn.execute("INSERT OR REPLACE INTO schema_version (version) VALUES (2)", [])
            .map_err(|e| e.to_string())?;
    }

    if current < 3 {
        // 添加 api_key_value 列（加密存储 API Key）
        let has_api_key_value: bool = conn
            .prepare("PRAGMA table_info(providers)")
            .map_err(|e| e.to_string())?
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .any(|col| col == "api_key_value");

        if !has_api_key_value {
            conn.execute("ALTER TABLE providers ADD COLUMN api_key_value TEXT", [])
                .map_err(|e| e.to_string())?;
        }

        conn.execute("INSERT OR REPLACE INTO schema_version (version) VALUES (3)", [])
            .map_err(|e| e.to_string())?;
    }

    if current < 4 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS session_stats (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                project         TEXT NOT NULL,
                session_id      TEXT NOT NULL,
                file_path       TEXT NOT NULL,
                file_size       INTEGER NOT NULL,
                file_mtime      TEXT NOT NULL,
                message_count   INTEGER DEFAULT 0,
                user_messages   INTEGER DEFAULT 0,
                assistant_msgs  INTEGER DEFAULT 0,
                input_tokens    INTEGER DEFAULT 0,
                output_tokens   INTEGER DEFAULT 0,
                models          TEXT DEFAULT '',
                tool_calls_json TEXT DEFAULT '[]',
                skill_calls_json TEXT DEFAULT '[]',
                agent_calls_json TEXT DEFAULT '[]',
                started_at      TEXT,
                ended_at        TEXT,
                duration_ms     INTEGER DEFAULT 0,
                synced_at       TEXT DEFAULT (datetime('now')),
                UNIQUE(project, session_id)
            );

            CREATE INDEX IF NOT EXISTS idx_session_stats_project ON session_stats(project);
            CREATE INDEX IF NOT EXISTS idx_session_stats_started ON session_stats(started_at);

            CREATE VIEW IF NOT EXISTS session_stats_daily AS
            SELECT
                date(started_at) AS date,
                project,
                COUNT(*)            AS session_count,
                SUM(message_count)  AS total_messages,
                SUM(input_tokens)   AS total_input_tokens,
                SUM(output_tokens)  AS total_output_tokens,
                SUM(duration_ms)    AS total_duration_ms
            FROM session_stats
            WHERE started_at IS NOT NULL
            GROUP BY date, project;",
        ).map_err(|e| e.to_string())?;

        conn.execute("INSERT OR REPLACE INTO schema_version (version) VALUES (4)", [])
            .map_err(|e| e.to_string())?;
    }

    if current < 5 {
        conn.execute_batch(
            "ALTER TABLE session_stats ADD COLUMN cache_read_tokens INTEGER DEFAULT 0;

             CREATE TABLE IF NOT EXISTS model_daily_stats (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                date            TEXT NOT NULL,
                model           TEXT NOT NULL,
                session_count   INTEGER DEFAULT 0,
                message_count   INTEGER DEFAULT 0,
                input_tokens    INTEGER DEFAULT 0,
                output_tokens   INTEGER DEFAULT 0,
                cache_read      INTEGER DEFAULT 0,
                UNIQUE(date, model)
             );

             CREATE INDEX IF NOT EXISTS idx_model_daily_date ON model_daily_stats(date);",
        )
        .map_err(|e| e.to_string())?;

        conn.execute("INSERT OR REPLACE INTO schema_version (version) VALUES (5)", [])
            .map_err(|e| e.to_string())?;
    }

    if current < 6 {
        // 逐条执行，忽略"列已存在"错误
        let _ = conn.execute_batch("ALTER TABLE session_stats ADD COLUMN skill_calls_json TEXT DEFAULT '[]'");
        let _ = conn.execute_batch("ALTER TABLE session_stats ADD COLUMN agent_calls_json TEXT DEFAULT '[]'");

        // 验证两列都存在
        let has_skill = conn.prepare("SELECT skill_calls_json FROM session_stats LIMIT 0").is_ok();
        let has_agent = conn.prepare("SELECT agent_calls_json FROM session_stats LIMIT 0").is_ok();

        if has_skill && has_agent {
            conn.execute("INSERT OR REPLACE INTO schema_version (version) VALUES (6)", [])
                .map_err(|e| e.to_string())?;
        } else {
            // 列没加全，重置版本让下次重试
            conn.execute("INSERT OR REPLACE INTO schema_version (version) VALUES (5)", [])
                .map_err(|e| e.to_string())?;
        }
    }

    if current < 7 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS mcp_config (
                id INTEGER PRIMARY KEY DEFAULT 1,
                port INTEGER NOT NULL DEFAULT 8339,
                auto_inject BOOLEAN NOT NULL DEFAULT 0,
                smartsearch_enabled BOOLEAN NOT NULL DEFAULT 1,
                academicsearch_enabled BOOLEAN NOT NULL DEFAULT 0,
                cleanfetch_enabled BOOLEAN NOT NULL DEFAULT 1,
                search_mode TEXT NOT NULL DEFAULT 'engine',
                tavily_api_key TEXT,
                jina_api_key TEXT,
                proxy_url TEXT,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            INSERT OR IGNORE INTO mcp_config (id) VALUES (1);

            CREATE TABLE IF NOT EXISTS mcp_api_stats (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool_name TEXT NOT NULL,
                api_name TEXT NOT NULL,
                success BOOLEAN NOT NULL,
                called_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_mcp_stats_tool ON mcp_api_stats(tool_name, called_at);
            CREATE INDEX IF NOT EXISTS idx_mcp_stats_month ON mcp_api_stats(called_at);",
        )
        .map_err(|e| e.to_string())?;

        conn.execute("INSERT OR REPLACE INTO schema_version (version) VALUES (7)", [])
            .map_err(|e| e.to_string())?;
    }

    if current < 8 {
        let _ = conn.execute_batch("ALTER TABLE session_stats ADD COLUMN parsed_lines INTEGER DEFAULT 0");
        let _ = conn.execute_batch("ALTER TABLE session_stats ADD COLUMN title TEXT DEFAULT ''");
        conn.execute("INSERT OR REPLACE INTO schema_version (version) VALUES (8)", [])
            .map_err(|e| e.to_string())?;
    }

    if current < 9 {
        let _ = conn.execute_batch(
            "ALTER TABLE providers ADD COLUMN compress_enabled BOOLEAN NOT NULL DEFAULT 0"
        );
        conn.execute("INSERT OR REPLACE INTO schema_version (version) VALUES (9)", [])
            .map_err(|e| e.to_string())?;
    }

    if current < 10 {
        let _ = conn.execute_batch("ALTER TABLE mcp_config ADD COLUMN baidu_api_key TEXT");
        conn.execute("INSERT OR REPLACE INTO schema_version (version) VALUES (10)", [])
            .map_err(|e| e.to_string())?;
    }

    if current < 11 {
        // 将所有 system 代理模式改为 direct（默认关闭本地路由代理）
        conn.execute("UPDATE providers SET proxy_mode = 'direct' WHERE proxy_mode = 'system'", [])
            .map_err(|e| e.to_string())?;
        conn.execute("INSERT OR REPLACE INTO schema_version (version) VALUES (11)", [])
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_creates_tables() {
        let conn = Connection::open_in_memory().unwrap();
        init_db_with_conn(&conn);

        for table in &["providers", "models", "request_logs", "schema_version"] {
            let count: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "table '{}' should exist", table);
        }
    }

    #[test]
    fn test_view_exists() {
        let conn = Connection::open_in_memory().unwrap();
        init_db_with_conn(&conn);
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='view' AND name='cost_daily'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_migration_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init_db_with_conn(&conn);
        init_db_with_conn(&conn);
    }

    #[test]
    fn dump_db_state() {
        let candidates = [
            std::path::PathBuf::from("target/debug/data/agentbox.db"),
            std::path::PathBuf::from("../target/debug/data/agentbox.db"),
        ];
        let Some(db_path) = candidates.iter().find(|p| p.exists()).cloned() else { return };
        let conn = Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();

        let total: i64 = conn.query_row("SELECT COUNT(*) FROM session_stats", [], |r| r.get(0)).unwrap();
        let with_tokens: i64 = conn.query_row("SELECT COUNT(*) FROM session_stats WHERE input_tokens > 0", [], |r| r.get(0)).unwrap();
        let projects_with_tokens: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT project) FROM session_stats WHERE input_tokens > 0", [], |r| r.get(0)
        ).unwrap();
        eprintln!("DB: {} rows, {} with tokens, {} projects with tokens", total, with_tokens, projects_with_tokens);

        let mut stmt = conn.prepare(
            "SELECT project, SUM(input_tokens), COUNT(*) FROM session_stats GROUP BY project ORDER BY SUM(input_tokens) DESC LIMIT 15"
        ).unwrap();
        for row in stmt.query_map([], |r| Ok((r.get::<_,String>(0)?, r.get::<_,i64>(1)?, r.get::<_,i64>(2)?))).unwrap().flatten() {
            eprintln!("  {}: {} tokens, {} sessions", row.0, row.1, row.2);
        }
    }

    #[test]
    fn test_migration_compress_enabled() {
        let conn = Connection::open_in_memory().unwrap();
        init_db_with_conn(&conn);

        let has_col: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('providers') WHERE name='compress_enabled'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            > 0;
        assert!(has_col, "providers table should have compress_enabled column");
    }
}
