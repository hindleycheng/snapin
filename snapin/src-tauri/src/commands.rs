// Snapin 闪贴 · Tauri 命令（全部放在子模块以规避 E0255）

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use image::GenericImageView;

// ============================================================
// 授权系统
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseStatus {
    pub activated: bool,
    pub email: Option<String>,
    pub plan: String,
}

impl Default for LicenseStatus {
    fn default() -> Self {
        Self {
            activated: false,
            email: None,
            plan: "free".into(),
        }
    }
}

fn license_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".snapin").join("license.json")
}

#[tauri::command]
pub fn get_license_status() -> LicenseStatus {
    let path = license_path();
    if path.exists() {
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(status) = serde_json::from_str::<LicenseStatus>(&data) {
                return status;
            }
        }
    }
    LicenseStatus::default()
}

#[tauri::command]
pub fn activate_license(email: String, license_key: String) -> Result<LicenseStatus, String> {
    if email.is_empty() || license_key.is_empty() {
        return Err("邮箱和授权码不能为空".into());
    }
    if !license_key.starts_with("SNPN-") {
        return Err("授权码格式不正确（应以 SNPN- 开头）".into());
    }
    let status = LicenseStatus {
        activated: true,
        email: Some(email),
        plan: "pro".into(),
    };
    let path = license_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&status).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(status)
}

#[tauri::command]
pub fn deactivate_license() -> Result<(), String> {
    let path = license_path();
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ============================================================
// 辅助
// ============================================================

fn screenshots_dir() -> PathBuf {
    let dir = dirs::picture_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("Snapin");
    fs::create_dir_all(&dir).ok();
    dir
}

fn gen_filename(prefix: &str) -> String {
    let now = chrono::Local::now();
    format!("{}_{}.png", prefix, now.format("%Y%m%d_%H%M%S"))
}

fn urlencoding_minimal(s: &str) -> String {
    s.replace('+', "%2B")
        .replace('/', "%2F")
        .replace('=', "%3D")
}

fn copy_image_to_clipboard(path: &PathBuf) -> Result<(), String> {
    use arboard::Clipboard;
    let img = image::open(path).map_err(|e| format!("{}", e))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut clipboard = Clipboard::new().map_err(|e| format!("{}", e))?;
    let img_data = arboard::ImageData {
        width: w as usize,
        height: h as usize,
        bytes: rgba.into_raw().into(),
    };
    clipboard.set_image(img_data).map_err(|e| format!("{}", e))?;
    Ok(())
}

fn primary_monitor() -> Result<xcap::Monitor, String> {
    use xcap::Monitor;
    let monitors = Monitor::all().map_err(|e| format!("获取显示器失败: {}", e))?;
    for m in &monitors {
        if m.is_primary().unwrap_or(false) {
            return Ok(m.clone());
        }
    }
    monitors
        .into_iter()
        .next()
        .ok_or_else(|| "找不到显示器".to_string())
}

// ============================================================
// 截图核心
// ============================================================

#[tauri::command]
pub fn capture_fullscreen() -> Result<String, String> {
    let primary = primary_monitor()?;
    let img = primary
        .capture_image()
        .map_err(|e| format!("截屏失败: {}", e))?;
    let save_path = screenshots_dir().join(gen_filename("Snap_full"));
    img.save(&save_path)
        .map_err(|e| format!("保存失败: {}", e))?;
    Ok(save_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn start_capture(app: AppHandle) -> Result<(), String> {
    use base64::Engine;
    let primary = primary_monitor()?;
    let img = primary.capture_image().map_err(|e| format!("{}", e))?;
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("{}", e))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    let width = primary.width().map_err(|e| format!("{}", e))?;
    let height = primary.height().map_err(|e| format!("{}", e))?;
    let capture_url = format!(
        "capture.html?w={}&h={}",
        width,
        height
    );
    if let Some(w) = app.get_webview_window("capture") {
        w.close().ok();
    }
    let _win = WebviewWindowBuilder::new(&app, "capture", WebviewUrl::App(capture_url.into()))
        .title("")
        .fullscreen(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .build()
        .map_err(|e| format!("创建截图窗口失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn save_region_capture(
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    img_base64: String,
) -> Result<String, String> {
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD
        .decode(&img_base64)
        .map_err(|e| format!("base64 解码失败: {}", e))?;
    let img = image::load_from_memory(&data).map_err(|e| format!("图片加载失败: {}", e))?;
    let cropped = img.crop_imm(x, y, w, h);
    let save_path = screenshots_dir().join(gen_filename("Snap"));
    cropped
        .save(&save_path)
        .map_err(|e| format!("保存失败: {}", e))?;
    copy_image_to_clipboard(&save_path).ok();
    Ok(save_path.to_string_lossy().to_string())
}

// ============================================================
// 贴图钉屏
// ============================================================

#[tauri::command]
pub async fn pin_from_clipboard(app: AppHandle) -> Result<(), String> {
    use arboard::Clipboard;
    use base64::Engine;
    let mut clipboard = Clipboard::new().map_err(|e| format!("剪贴板访问失败: {}", e))?;
    let img = clipboard
        .get_image()
        .map_err(|_| "剪贴板中没有图片".to_string())?;
    let rgba = image::RgbaImage::from_raw(
        img.width as u32,
        img.height as u32,
        img.bytes.into_owned(),
    )
    .ok_or("图片格式转换失败")?;
    let mut buf = Cursor::new(Vec::new());
    rgba.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("{}", e))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    let label = format!("pin_{}", chrono::Local::now().format("%H%M%S%3f"));
    let pin_url = format!(
        "pin.html?img={}&w={}&h={}",
        urlencoding_minimal(&b64),
        img.width,
        img.height
    );
    let _win = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(pin_url.into()))
        .title("Snapin Pin")
        .inner_size(img.width as f64, img.height as f64 + 28.0)
        .decorations(false)
        .always_on_top(true)
        .resizable(true)
        .build()
        .map_err(|e| format!("创建贴图窗口失败: {}", e))?;
    Ok(())
}

// ============================================================
// 拾色器
// ============================================================

#[tauri::command]
pub async fn start_color_picker(app: AppHandle) -> Result<(), String> {
    use base64::Engine;
    let primary = primary_monitor()?;
    let img = primary.capture_image().map_err(|e| format!("{}", e))?;
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("{}", e))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    if let Some(w) = app.get_webview_window("colorpicker") {
        w.close().ok();
    }
    let url = format!("colorpicker.html?img={}", urlencoding_minimal(&b64));
    let _win = WebviewWindowBuilder::new(&app, "colorpicker", WebviewUrl::App(url.into()))
        .title("")
        .fullscreen(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .build()
        .map_err(|e| format!("创建拾色器窗口失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn copy_color_to_clipboard(color: String) -> Result<(), String> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new().map_err(|e| format!("{}", e))?;
    clipboard.set_text(color).map_err(|e| format!("{}", e))?;
    Ok(())
}

// ============================================================
// 历史记录
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub filename: String,
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub created_at: String,
    pub has_annotations: bool,
}

fn db_path() -> PathBuf {
    let dir = dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("Snapin");
    fs::create_dir_all(&dir).ok();
    dir.join("history.json")
}

fn init_db() -> Result<(), String> {
    let path = db_path();
    if !path.exists() {
        fs::write(&path, "[]").map_err(|e| format!("{}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn add_history_entry(
    filename: String,
    path: String,
    width: u32,
    height: u32,
    has_annotations: bool,
) -> Result<(), String> {
    init_db()?;
    let db = db_path();
    let data = fs::read_to_string(&db).unwrap_or_else(|_| "[]".into());
    let mut entries: Vec<HistoryEntry> = serde_json::from_str(&data).unwrap_or_default();
    let id = entries.len() as i64 + 1;
    let created_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    entries.push(HistoryEntry {
        id,
        filename,
        path,
        width,
        height,
        created_at,
        has_annotations,
    });
    let json = serde_json::to_string_pretty(&entries).map_err(|e| format!("{}", e))?;
    fs::write(&db, json).map_err(|e| format!("{}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_history() -> Result<Vec<HistoryEntry>, String> {
    init_db()?;
    let db = db_path();
    let data = fs::read_to_string(&db).unwrap_or_else(|_| "[]".into());
    let mut entries: Vec<HistoryEntry> = serde_json::from_str(&data).unwrap_or_default();
    entries.reverse();
    Ok(entries)
}

#[tauri::command]
pub fn delete_history_entry(id: i64) -> Result<(), String> {
    let db = db_path();
    let data = fs::read_to_string(&db).unwrap_or_else(|_| "[]".into());
    let entries: Vec<HistoryEntry> = serde_json::from_str(&data).unwrap_or_default();
    let filtered: Vec<_> = entries.into_iter().filter(|e| e.id != id).collect();
    let json = serde_json::to_string_pretty(&filtered).map_err(|e| format!("{}", e))?;
    fs::write(&db, json).map_err(|e| format!("{}", e))?;
    Ok(())
}

// ============================================================
// 长截图
// ============================================================

#[tauri::command]
pub async fn start_scroll_capture(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("scroll_capture") {
        w.close().ok();
    }
    let _win = WebviewWindowBuilder::new(
        &app,
        "scroll_capture",
        WebviewUrl::App("scroll-capture.html".into()),
    )
    .title("Snapin · 长截图")
    .inner_size(540.0, 620.0)
    .decorations(true)
    .always_on_top(true)
    .resizable(false)
    .build()
    .map_err(|e| format!("创建长截图窗口失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn capture_scroll_frame(frame_index: u32) -> Result<String, String> {
    let primary = primary_monitor()?;
    let img = primary.capture_image().map_err(|e| format!("{}", e))?;
    let tmp_dir = std::env::temp_dir().join("snapin_scroll");
    fs::create_dir_all(&tmp_dir).ok();
    let path = tmp_dir.join(format!("frame_{:04}.png", frame_index));
    img.save(&path).map_err(|e| format!("{}", e))?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn simulate_scroll(pixels: i32) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let script = format!(
            "tell application \"System Events\" to scroll down {}",
            pixels / 100
        );
        Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| format!("滚动模拟失败: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let script = format!(
            "Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class Mouse{{[DllImport(\"user32.dll\")] public static extern void mouse_event(uint dwFlags, int dx, int dy, int dwData, IntPtr dwExtraInfo);}}';[Mouse]::mouse_event(0x0800,0,0,{},0)",
            -pixels
        );
        Command::new("powershell")
            .args(["-Command", &script])
            .output()
            .map_err(|e| format!("滚动模拟失败: {}", e))?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = pixels;
    }
    Ok(())
}

#[tauri::command]
pub fn stitch_scroll_frames(frame_paths: Vec<String>) -> Result<String, String> {
    use image::RgbaImage;
    if frame_paths.is_empty() {
        return Err("没有帧可拼接".into());
    }
    let mut images: Vec<image::DynamicImage> = Vec::new();
    for p in &frame_paths {
        let img = image::open(p).map_err(|e| format!("加载帧失败 {}: {}", p, e))?;
        images.push(img);
    }
    let first = &images[0];
    let (fw, fh) = first.dimensions();
    let overlap_estimate = (fh as f32 * 0.2) as u32;
    let effective_height = fh - overlap_estimate;
    let total_height = fh + (images.len() as u32 - 1) * effective_height;
    let mut result = RgbaImage::new(fw, total_height);
    for (i, img) in images.iter().enumerate() {
        let y_offset = i as u32 * effective_height;
        let rgba = img.to_rgba8();
        for (x, y, pixel) in rgba.enumerate_pixels() {
            if y_offset + y < total_height {
                result.put_pixel(x, y_offset + y, *pixel);
            }
        }
    }
    let save_path = screenshots_dir().join(gen_filename("Snap_long"));
    result
        .save(&save_path)
        .map_err(|e| format!("保存长图失败: {}", e))?;
    for p in &frame_paths {
        fs::remove_file(p).ok();
    }
    Ok(save_path.to_string_lossy().to_string())
}
