pub mod commands;
pub mod config;
pub mod db;
pub mod mcp;
pub mod presets;
pub mod proxy;
pub mod tray;

use std::sync::Arc;
use tauri::{Emitter, Manager};

/// 获取数据目录：跟随安装目录下的 data/ 子目录
fn resolve_data_dir() -> std::path::PathBuf {
    // 开发模式: exe 在 target/debug/，数据放那里即可
    // 发布模式: exe 在安装目录，数据放 <install_dir>/data/
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let data_dir = exe_dir.join("data");
    std::fs::create_dir_all(&data_dir).ok();
    data_dir
}

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
            commands::get_analytics_top_items,
            commands::metrics::check_usage_db,
            commands::metrics::get_model_detail_stats,
            commands::metrics::get_proxy_detail_stats,
            commands::search_skills_sh,
            commands::install_skill_from_repo,
            commands::uninstall_skill,
            // MCP
            commands::get_mcp_config,
            commands::save_mcp_config,
            commands::restart_mcp_server,
            commands::get_mcp_status,
            commands::get_mcp_stats,
            commands::inject_statusline,
            commands::remove_statusline,
            commands::discover_existing_providers,
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            // 数据目录跟随安装位置
            let data_dir = resolve_data_dir();
            let db_path = db::db_path(&data_dir);
            let conn = db::init_db(&db_path).expect("failed to init database");
            let db = Arc::new(std::sync::Mutex::new(conn));

            // MCP 服务器句柄
            let mcp_shutdown: Arc<std::sync::Mutex<Option<tokio::sync::watch::Sender<bool>>>> =
                Arc::new(std::sync::Mutex::new(None));

            // 注册 Tauri 状态
            app.manage(commands::AppState {
                db: db.clone(),
                mcp_shutdown: mcp_shutdown.clone(),
                global_memory_cache: Arc::new(std::sync::Mutex::new(Vec::new())),
            });

            // 延迟启动代理服务器（不阻塞 setup）
            let db_for_proxy = db.clone();
            let data_dir_for_proxy = data_dir.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                let client_pool = Arc::new(proxy::client_pool::ClientPool::new());

                // 初始化 CCR 压缩引擎（SQLite 持久化，失败时降级为 in-memory）
                let ccr_db_path = data_dir_for_proxy.join("ccr.db");
                let compression_engine = match proxy::compression::CompressionEngine::new_sqlite(&ccr_db_path) {
                    Ok(engine) => {
                        log::info!("CCR compression engine initialized at {}", ccr_db_path.display());
                        Arc::new(engine)
                    }
                    Err(e) => {
                        log::warn!("CCR SQLite init failed ({}), using in-memory backend", e);
                        Arc::new(proxy::compression::CompressionEngine::new_in_memory())
                    }
                };

                let state = proxy::engine::ProxyState {
                    db: db_for_proxy,
                    client_pool,
                    api_key_resolver: Arc::new(|env_name: &str| {
                        std::env::var(env_name).ok()
                    }),
                    compression_engine,
                };

                if let Err(e) = proxy::engine::start_proxy_server(18900, state).await {
                    log::error!("proxy server failed: {}", e);
                }
            });

            // 延迟启动 MCP 服务器
            let db_for_mcp = db.clone();
            let mcp_shutdown_for_mcp = mcp_shutdown.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

                let (tx, rx) = tokio::sync::watch::channel(false);
                {
                    let mut handle = mcp_shutdown_for_mcp.lock().unwrap();
                    *handle = Some(tx);
                }

                if let Err(e) = mcp::start_mcp_server(db_for_mcp, rx).await {
                    log::error!("MCP server failed: {}", e);
                }
            });

            // 定时同步：用独立 DB 连接，不与 UI 读操作争锁
            let sync_db_path = db_path.clone();
            let app_handle_for_sync = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                loop {
                    // 每次循环打开独立连接，同步完即关闭
                    match db::init_db(&sync_db_path) {
                        Ok(sync_conn) => {
                            if let Err(e) = commands::analytics::sync_session_stats(&sync_conn) {
                                log::warn!("session stats sync failed: {}", e);
                            } else {
                                let _ = app_handle_for_sync.emit("stats-synced", ());
                            }
                        }
                        Err(e) => log::warn!("sync: failed to open db: {}", e),
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                }
            });

            // 初始化全局记忆缓存（只读一个 QWEN.md）
            {
                let qwen_home = dirs::home_dir().unwrap_or_default().join(".qwen");
                let qwen_md = qwen_home.join("QWEN.md");
                if qwen_md.is_file() {
                    let content = std::fs::read_to_string(&qwen_md).unwrap_or_default();
                    let (name, desc) = commands::filesystem::parse_frontmatter(&content);
                    let mem_type = commands::filesystem::extract_frontmatter_field(&content, "type");
                    let state = app.state::<commands::AppState>();
                    let mut cache = state.global_memory_cache.lock().unwrap();
                    cache.push(commands::filesystem::MemoryFile {
                        name: name.unwrap_or_else(|| "QWEN".into()),
                        memory_type: mem_type.unwrap_or_else(|| "unknown".into()),
                        description: desc,
                        path: qwen_md.to_string_lossy().to_string(),
                    });
                }
            }

            // 拦截窗口关闭：点 X 隐藏到托盘而非退出
            if let Some(window) = app.get_webview_window("main") {
                let win = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win.hide();
                    }
                });
            }

            // 系统托盘
            if let Err(e) = tray::setup(app.handle()) {
                log::error!("Failed to setup tray: {}", e);
            }

            // 定时刷新托盘 MCP 状态（每 10 秒）
            let app_handle_for_tray = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await; // 等待 MCP 首次启动
                let running = tray::check_mcp_running(&app_handle_for_tray).await;
                tray::update_menu_status(&app_handle_for_tray, running);

                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    let running = tray::check_mcp_running(&app_handle_for_tray).await;
                    tray::update_menu_status(&app_handle_for_tray, running);
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running agentbox");
}
