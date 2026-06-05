use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

pub const TRAY_ID: &str = "agentbox";

/// 托盘菜单 ID
const MENU_SHOW: &str = "show_main";
const MENU_MCP_TOGGLE: &str = "mcp_toggle";
const MENU_QUIT: &str = "quit";

/// 创建托盘菜单。`mcp_running` 决定 MCP 项显示的文本。
fn build_menu(app: &tauri::AppHandle, mcp_running: bool) -> tauri::Result<Menu<tauri::Wry>> {
    let mcp_label = if mcp_running {
        "MCP 服务: 运行中 ✓"
    } else {
        "MCP 服务: 已停止"
    };

    let show = MenuItem::with_id(app, MENU_SHOW, "打开主界面", true, None::<&str>)?;
    let mcp = MenuItem::with_id(app, MENU_MCP_TOGGLE, mcp_label, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;

    Menu::with_items(app, &[&show, &mcp, &quit])
}

/// 构建系统托盘并返回 handle（供后续更新菜单）
pub fn setup(app: &tauri::AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app, false)?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("AgentBox")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle().clone();
                show_main_window(&app);
            }
        })
        .on_menu_event(|app, event| match event.id.0.as_str() {
            MENU_SHOW => show_main_window(app),
            MENU_MCP_TOGGLE => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = toggle_mcp_server(&app).await {
                        log::error!("Toggle MCP failed: {}", e);
                    }
                });
            }
            MENU_QUIT => app.exit(0),
            _ => {}
        });

    // 非 macOS 使用默认窗口图标
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}

/// 显示/恢复主窗口
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 切换 MCP 服务器状态
async fn toggle_mcp_server(app: &tauri::AppHandle) -> Result<(), String> {
    let running = check_mcp_running(app).await;

    if running {
        // 停止：发送 shutdown 信号（guard 在 sleep 前 drop）
        {
            let state = app.state::<crate::commands::AppState>();
            let mut handle = state.mcp_shutdown.lock().map_err(|e| e.to_string())?;
            if let Some(tx) = handle.take() {
                let _ = tx.send(true);
            }
        } // handle dropped here
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    } else {
        // 启动
        let config = {
            let state = app.state::<crate::commands::AppState>();
            let db = state.db.lock().map_err(|e| e.to_string())?;
            crate::mcp::get_config(&db)?
        };

        if !config.smartsearch_enabled
            && !config.academicsearch_enabled
            && !config.cleanfetch_enabled
        {
            return Err("所有 MCP 工具已禁用".into());
        }

        let listener = crate::mcp::server::bind_port(config.port).await?;
        let (tx, rx) = tokio::sync::watch::channel(false);
        {
            let state = app.state::<crate::commands::AppState>();
            let mut handle = state.mcp_shutdown.lock().map_err(|e| e.to_string())?;
            *handle = Some(tx);
        }
        let db = app.state::<crate::commands::AppState>().db.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::mcp::server::start_server(listener, db, rx).await {
                log::error!("MCP server error: {}", e);
            }
        });
    }

    // 刷新菜单状态
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let now_running = check_mcp_running(app).await;
    update_menu_status(app, now_running);

    Ok(())
}

/// TCP 检测 MCP 端口是否在监听
pub async fn check_mcp_running(app: &tauri::AppHandle) -> bool {
    let state = app.state::<crate::commands::AppState>();
    let port = match state.db.lock() {
        Ok(db) => crate::mcp::get_config(&db).map(|c| c.port).unwrap_or(8339),
        Err(_) => 8339,
    };
    tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
        .await
        .is_ok()
}

/// 更新托盘菜单中的 MCP 状态文本
pub fn update_menu_status(app: &tauri::AppHandle, mcp_running: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app, mcp_running) {
            let _ = tray.set_menu(Some(menu));
        }
        let tip = if mcp_running {
            "AgentBox · MCP 运行中"
        } else {
            "AgentBox · MCP 已停止"
        };
        let _ = tray.set_tooltip(Some(tip));
    }
}
