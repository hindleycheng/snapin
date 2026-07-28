// Snapin 闪贴 · 应用入口（Tauri 2 + Rust 1.88）
// 命令全部在 commands 子模块中，规避 E0255

pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::Manager;
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_license_status,
            commands::activate_license,
            commands::deactivate_license,
            commands::start_capture,
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
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // ============ 系统托盘 ============
            let menu = Menu::with_items(
                app,
                &[
                    &MenuItem::with_id(app, "capture", "区域截图  ⌘⇧A", true, None::<&str>)?,
                    &MenuItem::with_id(app, "fullscreen", "全屏截图  ⌘⇧F", true, None::<&str>)?,
                    &MenuItem::with_id(app, "pin", "贴图  F3", true, None::<&str>)?,
                    &MenuItem::with_id(app, "color", "拾色器  ⌘⇧C", true, None::<&str>)?,
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

            // ============ 全局快捷键 ============
            // macOS 用 Cmd，Windows/Linux 用 Ctrl
            #[cfg(target_os = "macos")]
            let mod_key = Modifiers::SUPER;
            #[cfg(not(target_os = "macos"))]
            let mod_key = Modifiers::CONTROL;

            let capture_sc = Shortcut::new(Some(mod_key | Modifiers::SHIFT), Code::KeyA);
            let pin_sc = Shortcut::new(None, Code::F3);
            let color_sc = Shortcut::new(Some(mod_key | Modifiers::SHIFT), Code::KeyC);

            let h1 = handle.clone();
            let h2 = handle.clone();
            let h3 = handle.clone();

            app.global_shortcut().on_shortcut(capture_sc, move |_, _, _| {
                let h = h1.clone();
                tauri::async_runtime::spawn(async move {
                    commands::start_capture(h).await.ok();
                });
            })?;

            app.global_shortcut().on_shortcut(pin_sc, move |_, _, _| {
                let h = h2.clone();
                tauri::async_runtime::spawn(async move {
                    commands::pin_from_clipboard(h).await.ok();
                });
            })?;

            app.global_shortcut().on_shortcut(color_sc, move |_, _, _| {
                let h = h3.clone();
                tauri::async_runtime::spawn(async move {
                    commands::start_color_picker(h).await.ok();
                });
            })?;

            println!("[Snapin] 启动完成 · 托盘+快捷键就绪");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("运行 Snapin 时出错");
}
