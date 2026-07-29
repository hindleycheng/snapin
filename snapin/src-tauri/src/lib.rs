// Snapin 闪贴 · 应用入口（Tauri 2 + Rust 1.88）
// 命令全部在 commands 子模块中，规避 E0255

pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::{Listener, Manager};
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_license_status,
            commands::activate_license,
            commands::deactivate_license,
            commands::check_screenshot_quota,
            commands::increment_screenshot_usage,
            commands::start_capture,
            commands::get_capture_data,
            commands::capture_fullscreen,
            commands::save_region_capture,
            commands::pin_from_clipboard,
            commands::start_color_picker,
            commands::copy_color_to_clipboard,
            commands::add_history_entry,
            commands::get_history,
            commands::delete_history_entry,
            commands::start_scroll_capture,
            commands::capture_scroll_frame,
            commands::simulate_scroll,
            commands::stitch_scroll_frames,
            commands::get_shortcuts,
            commands::set_shortcuts,
            commands::format_shortcut,
            commands::open_url,
            commands::start_recording,
            commands::stop_recording,
            commands::get_recording_status,
            commands::start_capture_with_record_mode,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            #[cfg(target_os = "macos")]
            {
                let _ = commands::ensure_screen_capture_access();
            }

            // ============ 系统托盘 ============
            let sc = commands::get_shortcuts();
            let cap_display = commands::format_shortcut(sc.capture.clone());
            let full_display = commands::format_shortcut(sc.fullscreen.clone());
            let pin_display = commands::format_shortcut(sc.pin.clone());
            let color_display = commands::format_shortcut(sc.color.clone());

            let menu = Menu::with_items(
                app,
                &[
                    &MenuItem::with_id(app, "capture", &format!("区域截图  {}", cap_display), true, None::<&str>)?,
                    &MenuItem::with_id(app, "fullscreen", &format!("全屏截图  {}", full_display), true, None::<&str>)?,
                    &MenuItem::with_id(app, "pin", &format!("贴图钉屏  {}", pin_display), true, None::<&str>)?,
                    &MenuItem::with_id(app, "color", &format!("拾色器  {}", color_display), true, None::<&str>)?,
                    &MenuItem::with_id(app, "sep1", "────────────", false, None::<&str>)?,
                    &MenuItem::with_id(app, "history", "历史记录", true, None::<&str>)?,
                    &MenuItem::with_id(app, "settings", "偏好设置…", true, None::<&str>)?,
                    &MenuItem::with_id(app, "sep2", "────────────", false, None::<&str>)?,
                    &MenuItem::with_id(app, "quit", "退出 Snapin", true, None::<&str>)?,
                ],
            )?;

            let tray_handle = handle.clone();
            let _tray = TrayIconBuilder::new()
                .tooltip("Snapin 闪贴")
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    let h = tray_handle.clone();
                    match event.id.as_ref() {
                        "capture" => {
                            let hc = h.clone();
                            tauri::async_runtime::spawn(async move {
                                commands::start_capture(hc).await.ok();
                            });
                        }
                        "fullscreen" => {
                            commands::capture_fullscreen().ok();
                        }
                        "pin" => {
                            let hc = h.clone();
                            tauri::async_runtime::spawn(async move {
                                commands::pin_from_clipboard(hc).await.ok();
                            });
                        }
                        "color" => {
                            let hc = h.clone();
                            tauri::async_runtime::spawn(async move {
                                commands::start_color_picker(hc).await.ok();
                            });
                        }
                        "history" | "settings" => {
                            if let Some(w) = app.get_webview_window("main") {
                                w.show().ok();
                                w.set_focus().ok();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            w.show().ok();
                            w.set_focus().ok();
                        }
                    }
                })
                .build(app)?;

            // ============ 全局快捷键（动态加载） ============
            register_all_shortcuts(&handle)?;

            // 监听快捷键变更事件，运行时重新注册
            let watch_handle = handle.clone();
            app.listen("shortcuts-changed", move |_| {
                let gs = watch_handle.app_handle().global_shortcut();
                gs.unregister_all().ok();
                register_all_shortcuts(watch_handle.app_handle()).ok();
            });
            let watch_handle = handle.clone();
            app.listen("shortcuts-changed", move |_| {
                let gs = watch_handle.app_handle().global_shortcut();
                gs.unregister_all().ok();
                register_all_shortcuts(watch_handle.app_handle()).ok();
            });

            println!("[Snapin] 启动完成 · 托盘+快捷键就绪");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("运行 Snapin 时出错");
}

/// 根据配置文件注册全部全局快捷键
fn register_all_shortcuts(handle: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let config = commands::get_shortcuts();

    register_one(handle, &config.capture, "capture")?;
    register_one(handle, &config.fullscreen, "fullscreen")?;
    register_one(handle, &config.pin, "pin")?;
    register_one(handle, &config.color, "color")?;

    Ok(())
}

fn register_one(
    handle: &tauri::AppHandle,
    accel: &str,
    action: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

    let shortcut: Shortcut = accel
        .parse()
        .map_err(|e| format!("无法解析快捷键 '{}': {}", accel, e))?;

    let h = handle.clone();
    let act = action.to_string();

    handle.global_shortcut().on_shortcut(shortcut, move |_, _, _| {
        let hc = h.clone();
        match act.as_str() {
            "capture" => {
                tauri::async_runtime::spawn(async move {
                    commands::start_capture(hc).await.ok();
                });
            }
            "fullscreen" => {
                commands::capture_fullscreen().ok();
            }
            "pin" => {
                tauri::async_runtime::spawn(async move {
                    commands::pin_from_clipboard(hc).await.ok();
                });
            }
            "color" => {
                tauri::async_runtime::spawn(async move {
                    commands::start_color_picker(hc).await.ok();
                });
            }
            _ => {}
        }
    })?;

    Ok(())
}
