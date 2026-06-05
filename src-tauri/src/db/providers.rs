use rusqlite::Connection;
use serde::{Deserialize, Serialize};

// ── Provider ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: i64,
    pub name: String,
    pub base_url: String,
    pub api_key_env: String,
    pub proxy_mode: String,
    pub proxy_url: Option<String>,
    pub auth_header: Option<String>,
    pub api_key_value: Option<String>,
    pub billing_type: String,
    pub is_active: bool,
    pub compress_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProvider {
    pub name: String,
    pub base_url: String,
    pub api_key_env: String,
    pub proxy_mode: Option<String>,
    pub proxy_url: Option<String>,
    pub auth_header: Option<String>,
    pub api_key_value: Option<String>,
    pub billing_type: Option<String>,
    pub compress_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProvider {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub proxy_mode: Option<String>,
    pub proxy_url: Option<String>,
    pub auth_header: Option<String>,
    pub api_key_value: Option<String>,
    pub billing_type: Option<String>,
    pub is_active: Option<bool>,
    pub compress_enabled: Option<bool>,
}

pub fn list_providers(conn: &Connection) -> Result<Vec<Provider>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, base_url, api_key_env, proxy_mode, proxy_url, auth_header, api_key_value, billing_type, is_active, compress_enabled, created_at, updated_at FROM providers ORDER BY id")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Provider {
                id: row.get(0)?,
                name: row.get(1)?,
                base_url: row.get(2)?,
                api_key_env: row.get(3)?,
                proxy_mode: row.get(4)?,
                proxy_url: row.get(5)?,
                auth_header: row.get(6)?,
                api_key_value: row.get(7)?,
                billing_type: row.get(8)?,
                is_active: row.get(9)?,
                compress_enabled: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn get_provider(conn: &Connection, id: i64) -> Result<Provider, String> {
    conn.query_row(
        "SELECT id, name, base_url, api_key_env, proxy_mode, proxy_url, auth_header, api_key_value, billing_type, is_active, compress_enabled, created_at, updated_at FROM providers WHERE id = ?1",
        [id],
        |row| {
            Ok(Provider {
                id: row.get(0)?,
                name: row.get(1)?,
                base_url: row.get(2)?,
                api_key_env: row.get(3)?,
                proxy_mode: row.get(4)?,
                proxy_url: row.get(5)?,
                auth_header: row.get(6)?,
                api_key_value: row.get(7)?,
                billing_type: row.get(8)?,
                is_active: row.get(9)?,
                compress_enabled: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        },
    ).map_err(|e| e.to_string())
}

pub fn create_provider(conn: &Connection, p: &CreateProvider) -> Result<Provider, String> {
    let proxy_mode = p.proxy_mode.as_deref().unwrap_or("system");
    let billing_type = p.billing_type.as_deref().unwrap_or("pay_per_use");
    let compress_enabled = p.compress_enabled.unwrap_or(false);
    conn.execute(
        "INSERT INTO providers (name, base_url, api_key_env, proxy_mode, proxy_url, auth_header, api_key_value, billing_type, compress_enabled) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![p.name, p.base_url, p.api_key_env, proxy_mode, p.proxy_url, p.auth_header, p.api_key_value, billing_type, compress_enabled],
    ).map_err(|e| e.to_string())?;

    let id = conn.last_insert_rowid();
    get_provider(conn, id)
}

pub fn update_provider(conn: &Connection, id: i64, u: &UpdateProvider) -> Result<Provider, String> {
    let current = get_provider(conn, id)?;

    conn.execute(
        "UPDATE providers SET
            name = COALESCE(?1, name),
            base_url = COALESCE(?2, base_url),
            api_key_env = COALESCE(?3, api_key_env),
            proxy_mode = COALESCE(?4, proxy_mode),
            proxy_url = ?5,
            auth_header = ?6,
            api_key_value = ?7,
            billing_type = COALESCE(?8, billing_type),
            is_active = COALESCE(?9, is_active),
            compress_enabled = COALESCE(?10, compress_enabled),
            updated_at = datetime('now')
        WHERE id = ?11",
        rusqlite::params![
            u.name,
            u.base_url,
            u.api_key_env,
            u.proxy_mode,
            u.proxy_url.as_ref().or(current.proxy_url.as_ref()),
            u.auth_header.as_ref().or(current.auth_header.as_ref()),
            u.api_key_value.as_ref().or(current.api_key_value.as_ref()),
            u.billing_type,
            u.is_active,
            u.compress_enabled,
            id
        ],
    )
    .map_err(|e| e.to_string())?;

    get_provider(conn, id)
}

pub fn delete_provider(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM providers WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Model ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: i64,
    pub provider_id: i64,
    pub model_id: String,
    pub display_name: Option<String>,
    pub auth_type: String,
    pub is_default: bool,
    pub config_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateModel {
    pub provider_id: i64,
    pub model_id: String,
    pub display_name: Option<String>,
    pub auth_type: String,
    pub is_default: Option<bool>,
    pub config_json: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateModel {
    pub display_name: Option<String>,
    pub auth_type: Option<String>,
    pub is_default: Option<bool>,
    pub config_json: Option<String>,
}

pub fn list_models(conn: &Connection, provider_id: Option<i64>) -> Result<Vec<Model>, String> {
    let (sql, params): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match provider_id {
        Some(pid) => (
            "SELECT id, provider_id, model_id, display_name, auth_type, is_default, config_json, created_at FROM models WHERE provider_id = ?1 ORDER BY id",
            vec![Box::new(pid)],
        ),
        None => (
            "SELECT id, provider_id, model_id, display_name, auth_type, is_default, config_json, created_at FROM models ORDER BY provider_id, id",
            vec![],
        ),
    };

    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok(Model {
                id: row.get(0)?,
                provider_id: row.get(1)?,
                model_id: row.get(2)?,
                display_name: row.get(3)?,
                auth_type: row.get(4)?,
                is_default: row.get(5)?,
                config_json: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn get_model(conn: &Connection, id: i64) -> Result<Model, String> {
    conn.query_row(
        "SELECT id, provider_id, model_id, display_name, auth_type, is_default, config_json, created_at FROM models WHERE id = ?1",
        [id],
        |row| {
            Ok(Model {
                id: row.get(0)?,
                provider_id: row.get(1)?,
                model_id: row.get(2)?,
                display_name: row.get(3)?,
                auth_type: row.get(4)?,
                is_default: row.get(5)?,
                config_json: row.get(6)?,
                created_at: row.get(7)?,
            })
        },
    ).map_err(|e| e.to_string())
}

/// 按 model_id 查找路由信息（代理引擎用）
/// auth_type 存储为 JSON 数组字符串，如 `["openai","anthropic"]`
/// 路由优先级：billing_type(plan > pay_per_use) > last_success_at > is_default > id
pub fn find_model_route(
    conn: &Connection,
    model_id: &str,
) -> Result<Option<ModelRoute>, String> {
    let result = conn.query_row(
        "SELECT m.id, m.model_id, m.auth_type, m.is_default, m.config_json,
                p.id, p.name, p.base_url, p.api_key_env, p.proxy_mode, p.proxy_url, p.auth_header, p.billing_type, p.compress_enabled
         FROM models m
         JOIN providers p ON m.provider_id = p.id
         WHERE m.model_id = ?1 AND p.is_active = 1
         ORDER BY CASE p.billing_type WHEN 'plan' THEN 0 ELSE 1 END,
                  m.last_success_at DESC NULLS LAST,
                  m.is_default DESC,
                  m.id DESC
         LIMIT 1",
        [model_id],
        |row| {
            Ok(ModelRoute {
                model_db_id: row.get(0)?,
                model_id: row.get(1)?,
                auth_type: row.get(2)?,
                is_default: row.get(3)?,
                config_json: row.get(4)?,
                provider_id: row.get(5)?,
                provider_name: row.get(6)?,
                base_url: row.get(7)?,
                api_key_env: row.get(8)?,
                proxy_mode: row.get(9)?,
                proxy_url: row.get(10)?,
                auth_header: row.get(11)?,
                billing_type: row.get(12)?,
                compress_enabled: row.get(13)?,
            })
        },
    );

    match result {
        Ok(route) => Ok(Some(route)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// 查找同一 model_id 的所有路由候选（用于加权路由）
pub fn find_all_routes(
    conn: &Connection,
    model_id: &str,
) -> Result<Vec<ModelRoute>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.model_id, m.auth_type, m.is_default, m.config_json,
                    p.id, p.name, p.base_url, p.api_key_env, p.proxy_mode, p.proxy_url, p.auth_header, p.billing_type, p.compress_enabled
             FROM models m
             JOIN providers p ON m.provider_id = p.id
             WHERE m.model_id = ?1 AND p.is_active = 1
             ORDER BY CASE p.billing_type WHEN 'plan' THEN 0 ELSE 1 END,
                      m.last_success_at DESC NULLS LAST",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([model_id], |row| {
            Ok(ModelRoute {
                model_db_id: row.get(0)?,
                model_id: row.get(1)?,
                auth_type: row.get(2)?,
                is_default: row.get(3)?,
                config_json: row.get(4)?,
                provider_id: row.get(5)?,
                provider_name: row.get(6)?,
                base_url: row.get(7)?,
                api_key_env: row.get(8)?,
                proxy_mode: row.get(9)?,
                proxy_url: row.get(10)?,
                auth_header: row.get(11)?,
                billing_type: row.get(12)?,
                compress_enabled: row.get(13)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// 请求成功后更新 last_success_at，用于下次路由优先选择
pub fn touch_model_success(conn: &Connection, model_db_id: i64) {
    let _ = conn.execute(
        "UPDATE models SET last_success_at = datetime('now') WHERE id = ?1",
        [model_db_id],
    );
}

#[derive(Debug, Clone)]
pub struct ModelRoute {
    pub model_db_id: i64,
    pub model_id: String,
    pub auth_type: String,
    pub is_default: bool,
    pub config_json: Option<String>,
    pub provider_id: i64,
    pub provider_name: String,
    pub base_url: String,
    pub api_key_env: String,
    pub proxy_mode: String,
    pub proxy_url: Option<String>,
    pub auth_header: Option<String>,
    pub billing_type: String,
    pub compress_enabled: bool,
}

pub fn create_model(conn: &Connection, m: &CreateModel) -> Result<Model, String> {
    let is_default = m.is_default.unwrap_or(false);
    conn.execute(
        "INSERT INTO models (provider_id, model_id, display_name, auth_type, is_default, config_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![m.provider_id, m.model_id, m.display_name, m.auth_type, is_default, m.config_json],
    ).map_err(|e| e.to_string())?;

    let id = conn.last_insert_rowid();
    get_model(conn, id)
}

pub fn update_model(conn: &Connection, id: i64, u: &UpdateModel) -> Result<Model, String> {
    conn.execute(
        "UPDATE models SET
            display_name = COALESCE(?1, display_name),
            auth_type = COALESCE(?2, auth_type),
            is_default = COALESCE(?3, is_default),
            config_json = COALESCE(?4, config_json)
        WHERE id = ?5",
        rusqlite::params![u.display_name, u.auth_type, u.is_default, u.config_json, id],
    )
    .map_err(|e| e.to_string())?;

    get_model(conn, id)
}

pub fn delete_model(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM models WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
