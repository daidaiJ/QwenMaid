pub mod engines;
pub mod protocol;
pub mod server;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub port: u16,
    pub auto_inject: bool,
    pub smartsearch_enabled: bool,
    pub academicsearch_enabled: bool,
    pub cleanfetch_enabled: bool,
    pub search_mode: String,
    pub tavily_api_key: Option<String>,
    pub jina_api_key: Option<String>,
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpStats {
    pub monthly_total: i64,
    pub monthly_success: i64,
    pub by_tool: Vec<ToolStats>,
    pub by_api: Vec<ApiStats>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolStats {
    pub tool_name: String,
    pub total: i64,
    pub success: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiStats {
    pub api_name: String,
    pub total: i64,
    pub success: i64,
}

pub fn get_config(db: &Connection) -> Result<McpConfig, String> {
    db.query_row(
        "SELECT port, auto_inject, smartsearch_enabled, academicsearch_enabled, \
         cleanfetch_enabled, search_mode, tavily_api_key, jina_api_key, proxy_url \
         FROM mcp_config WHERE id = 1",
        [],
        |row| {
            Ok(McpConfig {
                port: row.get(0)?,
                auto_inject: row.get(1)?,
                smartsearch_enabled: row.get(2)?,
                academicsearch_enabled: row.get(3)?,
                cleanfetch_enabled: row.get(4)?,
                search_mode: row.get(5)?,
                tavily_api_key: row.get(6)?,
                jina_api_key: row.get(7)?,
                proxy_url: row.get(8)?,
            })
        },
    )
    .map_err(|e| format!("Failed to read MCP config: {}", e))
}

pub fn save_config(db: &Connection, config: &McpConfig) -> Result<(), String> {
    db.execute(
        "UPDATE mcp_config SET port = ?1, auto_inject = ?2, smartsearch_enabled = ?3, \
         academicsearch_enabled = ?4, cleanfetch_enabled = ?5, search_mode = ?6, \
         tavily_api_key = ?7, jina_api_key = ?8, proxy_url = ?9, \
         updated_at = datetime('now') WHERE id = 1",
        rusqlite::params![
            config.port,
            config.auto_inject,
            config.smartsearch_enabled,
            config.academicsearch_enabled,
            config.cleanfetch_enabled,
            config.search_mode,
            config.tavily_api_key,
            config.jina_api_key,
            config.proxy_url,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_stats(db: &Connection) -> Result<McpStats, String> {
    let monthly_total: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM mcp_api_stats \
             WHERE called_at >= date('now', 'start of month')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let monthly_success: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM mcp_api_stats \
             WHERE called_at >= date('now', 'start of month') AND success = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut stmt = db
        .prepare(
            "SELECT tool_name, COUNT(*), SUM(CASE WHEN success THEN 1 ELSE 0 END) \
             FROM mcp_api_stats WHERE called_at >= date('now', 'start of month') \
             GROUP BY tool_name",
        )
        .map_err(|e| e.to_string())?;
    let by_tool: Vec<ToolStats> = stmt
        .query_map([], |row| {
            Ok(ToolStats {
                tool_name: row.get(0)?,
                total: row.get(1)?,
                success: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut stmt = db
        .prepare(
            "SELECT api_name, COUNT(*), SUM(CASE WHEN success THEN 1 ELSE 0 END) \
             FROM mcp_api_stats WHERE called_at >= date('now', 'start of month') \
             GROUP BY api_name",
        )
        .map_err(|e| e.to_string())?;
    let by_api: Vec<ApiStats> = stmt
        .query_map([], |row| {
            Ok(ApiStats {
                api_name: row.get(0)?,
                total: row.get(1)?,
                success: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(McpStats {
        monthly_total,
        monthly_success,
        by_tool,
        by_api,
    })
}

pub async fn start_mcp_server(
    db: Arc<Mutex<Connection>>,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let config = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        get_config(&conn)?
    };

    if !config.smartsearch_enabled
        && !config.academicsearch_enabled
        && !config.cleanfetch_enabled
    {
        log::info!("All MCP tools disabled, skipping server start");
        return Ok(());
    }

    server::start_server(config.port, db, shutdown_rx).await
}
