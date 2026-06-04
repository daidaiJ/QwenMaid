pub mod commands;
pub mod config;
pub mod db;
pub mod presets;
pub mod proxy;

use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::list_providers,
            commands::get_provider,
            commands::create_provider,
            commands::update_provider,
            commands::delete_provider,
            commands::list_models,
            commands::get_model,
            commands::create_model,
            commands::update_model,
            commands::delete_model,
            commands::read_settings,
            commands::write_settings_field,
            commands::get_env_vars,
            commands::reveal_in_explorer,
            commands::get_qwen_paths,
            commands::sync_config_to_settings,
            commands::preview_sync_config,
            commands::installer::detect_qwen_version,
            commands::installer::check_latest_qwen_version,
            commands::installer::detect_node_version,
            commands::installer::detect_npm_version,
            commands::installer::install_qwen_code,
            commands::installer::update_qwen_code,
            commands::installer::configure_npm_mirror,
            commands::installer::get_npm_mirror,
            // filesystem
            commands::filesystem::list_skills,
            commands::filesystem::read_skill_content,
            commands::filesystem::delete_skill,
            commands::filesystem::write_skill,
            commands::filesystem::list_projects,
            commands::filesystem::list_sessions,
            commands::filesystem::read_session,
            commands::filesystem::list_memories,
            commands::filesystem::read_memory,
            commands::filesystem::write_memory,
            commands::filesystem::delete_memory,
            commands::filesystem::list_agents,
            commands::filesystem::read_agent,
            commands::filesystem::write_agent,
            commands::filesystem::delete_agent,
            commands::filesystem::list_extensions,
            commands::filesystem::read_extension_detail,
            commands::filesystem::toggle_extension,
            commands::filesystem::delete_extension,
            commands::filesystem::write_extension_context,
            commands::filesystem::get_index,
            commands::filesystem::get_session_detail,
            commands::filesystem::get_session_messages_paged,
            commands::sync_session_stats,
            commands::get_analytics_summary,
            commands::metrics::check_usage_db,
            commands::metrics::get_model_detail_stats,
            commands::search_skills_sh,
            commands::install_skill_from_repo,
            commands::uninstall_skill,
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            // 数据库初始化（轻量，可在 setup 中同步完成）
            let app_data = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            std::fs::create_dir_all(&app_data).ok();

            let db_path = db::db_path(&app_data);
            let conn = db::init_db(&db_path).expect("failed to init database");
            let db = Arc::new(std::sync::Mutex::new(conn));

            // 注册 Tauri 状态
            app.manage(commands::AppState { db: db.clone() });

            // 延迟启动代理服务器（不阻塞 setup）
            let db_for_proxy = db.clone();
            tauri::async_runtime::spawn(async move {
                // 延迟 500ms，让窗口先渲染
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                let client_pool = Arc::new(proxy::client_pool::ClientPool::new());
                let state = proxy::engine::ProxyState {
                    db: db_for_proxy,
                    client_pool,
                    api_key_resolver: Arc::new(|env_name: &str| {
                        std::env::var(env_name).ok()
                    }),
                };

                if let Err(e) = proxy::engine::start_proxy_server(18900, state).await {
                    log::error!("proxy server failed: {}", e);
                }
            });

            // 定时同步会话统计数据（每 5 分钟）
            let db_for_sync = db.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                loop {
                    {
                        let Ok(conn) = db_for_sync.lock() else { continue };
                        if let Err(e) = commands::analytics::sync_session_stats(&conn) {
                            log::warn!("session stats sync failed: {}", e);
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running agentbox");
}
