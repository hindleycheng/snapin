// Snapin 闪贴 · Tauri 命令（全部放在子模块以规避 E0255）

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use image::GenericImageView;

/// 暂存最近一次截图的 base64，供 capture.html 取回
static LAST_CAPTURE_B64: Mutex<Option<String>> = Mutex::new(None);

// ============================================================
// 授权系统（在线验证 + 设备绑定）
// ============================================================

/// 服务端地址
const LICENSE_SERVER: &str = "https://www.hfyz.cloud";
/// 免费版每日截图次数
const FREE_DAILY_LIMIT: i32 = 3;

#[cfg(target_os = "macos")]
pub(crate) fn ensure_screen_capture_access() -> Result<(), String> {
    let access = core_graphics::access::ScreenCaptureAccess::default();
    if access.preflight() {
        return Ok(());
    }

    access.request();
    Err("Snapin 尚未获得屏幕录制权限。请在「系统设置 → 隐私与安全性 → 屏幕与系统录音」中允许 Snapin，然后完全退出并重新打开应用。".to_string())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn ensure_screen_capture_access() -> Result<(), String> {
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseStatus {
    pub activated: bool,
    pub email: Option<String>,
    pub plan: String,
    #[serde(default)]
    pub device_id: Option<String>,
}

impl Default for LicenseStatus {
    fn default() -> Self {
        Self {
            activated: false,
            email: None,
            plan: "free".into(),
            device_id: None,
        }
    }
}

fn snapin_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = home.join(".snapin");
    fs::create_dir_all(&dir).ok();
    dir
}

fn license_path() -> PathBuf {
    snapin_dir().join("license.json")
}

fn usage_path() -> PathBuf {
    snapin_dir().join("usage.json")
}

/// 生成设备指纹：主机名 + CPU 核心数 + macOS 序列号（或 Windows machine GUID）
fn get_device_id() -> String {
    let mut parts = Vec::new();

    // 主机名
    if let Ok(hostname) = hostname::get() {
        parts.push(hostname.to_string_lossy().to_string());
    }

    // CPU 核心数
    parts.push(num_cpus::get().to_string());

    #[cfg(target_os = "macos")]
    {
        // macOS: 硬件 UUID
        if let Ok(output) = std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().find(|l| l.contains("IOPlatformUUID")) {
                if let Some(uuid) = line.split('"').nth(3) {
                    parts.push(uuid.to_string());
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: MachineGuid from registry
        if let Ok(output) = std::process::Command::new("reg")
            .args(["query", r"HKLM\SOFTWARE\Microsoft\Cryptography", "/v", "MachineGuid"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().find(|l| l.contains("MachineGuid")) {
                if let Some(guid) = line.split_whitespace().last() {
                    parts.push(guid.to_string());
                }
            }
        }
    }

    // 组合后取简单 hash，避免泄露太多信息
    let combined = parts.join("|");
    let mut hash: u64 = 0;
    for byte in combined.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    format!("dev_{:016x}", hash)
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
pub async fn activate_license(email: String, license_key: String) -> Result<LicenseStatus, String> {
    if email.is_empty() || license_key.is_empty() {
        return Err("邮箱和授权码不能为空".into());
    }
    if !license_key.starts_with("SNPN-") {
        return Err("授权码格式不正确（应以 SNPN- 开头）".into());
    }

    let device_id = get_device_id();

    // 调服务端验证
    let url = format!("{}/api/license/activate", LICENSE_SERVER);
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "email": email,
            "license_key": license_key,
            "device_id": device_id,
        }))
        .send()
        .await
        .map_err(|e| format!("无法连接授权服务器：{}\n请检查网络后重试", e))?;

    if !resp.status().is_success() {
        let error_msg = resp
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
            .unwrap_or_else(|| "授权验证失败".to_string());
        return Err(error_msg);
    }

    // 验证成功，写入本地
    let status = LicenseStatus {
        activated: true,
        email: Some(email),
        plan: "pro".into(),
        device_id: Some(device_id),
    };
    let json = serde_json::to_string_pretty(&status).map_err(|e| e.to_string())?;
    fs::write(license_path(), json).map_err(|e| e.to_string())?;
    Ok(status)
}

#[tauri::command]
pub async fn deactivate_license() -> Result<(), String> {
    // 先读本地状态
    let path = license_path();
    if !path.exists() {
        return Ok(());
    }

    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(status) = serde_json::from_str::<LicenseStatus>(&data) {
            if status.activated {
                if let (Some(email), Some(key), Some(device_id)) =
                    (&status.email, status.device_id.as_ref(), &status.device_id)
                {
                    // 调服务端解绑设备
                    let url = format!("{}/api/license/deactivate", LICENSE_SERVER);
                    let client = reqwest::Client::new();
                    let _ = client
                        .post(&url)
                        .json(&serde_json::json!({
                            "email": email,
                            "license_key": key,
                            "device_id": device_id,
                        }))
                        .send()
                        .await;
                }
            }
        }
    }

    // 删除本地文件
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ============================================================
// 每日次数限制
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DailyUsage {
    date: String,
    count: i32,
}

/// 检查是否还能截图，返回 (允许, 剩余次数, 是否已激活)
#[tauri::command]
pub fn check_screenshot_quota() -> Result<(bool, i32, bool), String> {
    let status = get_license_status();
    if status.activated {
        return Ok((true, -1, true)); // 已激活，无限
    }

    let path = usage_path();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let usage: DailyUsage = if path.exists() {
        let data = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or(DailyUsage {
            date: today.clone(),
            count: 0,
        })
    } else {
        DailyUsage {
            date: today.clone(),
            count: 0,
        }
    };

    // 日期不同则重置
    if usage.date != today {
        let fresh = DailyUsage {
            date: today,
            count: 0,
        };
        let json = serde_json::to_string_pretty(&fresh).map_err(|e| e.to_string())?;
        fs::write(&path, json).map_err(|e| e.to_string())?;
        return Ok((true, FREE_DAILY_LIMIT, false));
    }

    let remaining = FREE_DAILY_LIMIT - usage.count;
    Ok((remaining > 0, remaining, false))
}

/// 截图后调用，记录已使用一次
#[tauri::command]
pub fn increment_screenshot_usage() -> Result<(), String> {
    let status = get_license_status();
    if status.activated {
        return Ok(());
    }

    let path = usage_path();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let mut usage: DailyUsage = if path.exists() {
        let data = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or(DailyUsage {
            date: today.clone(),
            count: 0,
        })
    } else {
        DailyUsage {
            date: today.clone(),
            count: 0,
        }
    };

    if usage.date != today {
        usage.date = today;
        usage.count = 0;
    }
    usage.count += 1;

    let json = serde_json::to_string_pretty(&usage).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
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

/// 在系统默认浏览器中打开一个 URL
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("打开浏览器失败: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|e| format!("打开浏览器失败: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("打开浏览器失败: {}", e))?;
    }
    Ok(())
}

// ============================================================
// 快捷键自定义
// ============================================================

/// 单个快捷键配置
/// accel 格式：Tauri 加速器字符串，如 "CmdOrCtrl+Shift+A"、"F3"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutConfig {
    pub capture: String,    // 区域截图
    pub fullscreen: String, // 全屏截图
    pub pin: String,        // 贴图钉屏
    pub color: String,      // 拾色器
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            capture: "CmdOrCtrl+Shift+A".into(),
            fullscreen: "CmdOrCtrl+Shift+F".into(),
            pin: "F3".into(),
            color: "CmdOrCtrl+Shift+C".into(),
        }
    }
}

fn shortcuts_path() -> PathBuf {
    snapin_dir().join("shortcuts.json")
}

/// 读取快捷键配置（不存在则返回默认值）
#[tauri::command]
pub fn get_shortcuts() -> ShortcutConfig {
    let path = shortcuts_path();
    if path.exists() {
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<ShortcutConfig>(&data) {
                return cfg;
            }
        }
    }
    ShortcutConfig::default()
}

/// 保存快捷键配置，并通知前端刷新
#[tauri::command]
pub fn set_shortcuts(app: AppHandle, config: ShortcutConfig) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(shortcuts_path(), json).map_err(|e| e.to_string())?;

    // 通知 lib.rs 重新注册快捷键
    app.emit("shortcuts-changed", &config).map_err(|e| e.to_string())?;
    Ok(())
}

/// 将加速器字符串格式化为用户可读的显示文本
#[tauri::command]
pub fn format_shortcut(accel: String) -> String {
    #[cfg(target_os = "macos")]
    {
        return accel
            .replace("CmdOrCtrl", "⌘")
            .replace("Command", "⌘")
            .replace("Control", "⌃")
            .replace("Shift", "⇧")
            .replace("Alt", "⌥")
            .replace("Super", "⌘")
            .replace("+", "");
    }
    #[cfg(not(target_os = "macos"))]
    {
        return accel
            .replace("CmdOrCtrl", "Ctrl")
            .replace("Super", "Win")
            .replace("+", " + ");
    }
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

// ============================================================
// 屏幕录制
// ============================================================

static IS_RECORDING: AtomicBool = AtomicBool::new(false);
static FRAME_COUNT: AtomicU32 = AtomicU32::new(0);

fn record_dir() -> PathBuf {
    std::env::temp_dir().join("snapin_record")
}

/// 录屏数据用于 start_capture 和 stop_recording 之间共享
static RECORD_BOUNDS: Mutex<Option<(u32, u32, u32, u32)>> = Mutex::new(None);

#[tauri::command]
pub fn start_recording(fps: u32, x: u32, y: u32, w: u32, h: u32) -> Result<(), String> {
    if IS_RECORDING.load(Ordering::SeqCst) {
        return Err("已经在录制中".to_string());
    }
    let dir = record_dir();
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| format!("创建临时目录失败: {}", e))?;
    IS_RECORDING.store(true, Ordering::SeqCst);
    FRAME_COUNT.store(0, Ordering::SeqCst);

    // 保存选区边界
    if let Ok(mut b) = RECORD_BOUNDS.lock() {
        *b = Some((x, y, w, h));
    }

    let interval = std::time::Duration::from_millis((1000 / fps.max(1).min(30)) as u64);
    std::thread::spawn(move || {
        while IS_RECORDING.load(Ordering::SeqCst) {
            if let Ok(monitors) = xcap::Monitor::all() {
                if let Some(m) = monitors.first() {
                    if let Ok(img) = m.capture_image() {
                        let (iw, ih) = (img.width() as u32, img.height() as u32);
                        let cx = x.min(iw.saturating_sub(1));
                        let cy = y.min(ih.saturating_sub(1));
                        let cw = w.min(iw.saturating_sub(cx));
                        let ch = h.min(ih.saturating_sub(cy));
                        let cropped = image::DynamicImage::ImageRgba8(img)
                            .crop_imm(cx, cy, cw, ch);
                        let idx = FRAME_COUNT.fetch_add(1, Ordering::SeqCst);
                        let path = dir.join(format!("frame_{:05}.png", idx));
                        if cropped.save(&path).is_err() {
                            FRAME_COUNT.fetch_sub(1, Ordering::SeqCst);
                        }
                    }
                }
            }
            std::thread::sleep(interval);
        }
    });
    Ok(())
}

#[tauri::command]
pub fn stop_recording() -> Result<String, String> {
    if !IS_RECORDING.load(Ordering::SeqCst) {
        return Err("当前没有在录制".to_string());
    }
    IS_RECORDING.store(false, Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(500)); // 等待最后一帧写入完成
    let count = FRAME_COUNT.load(Ordering::SeqCst);
    let dir = record_dir();
    if count == 0 {
        let _ = fs::remove_dir_all(&dir);
        return Err("没有录制到任何帧".to_string());
    }
    let output = screenshots_dir().join(format!(
        "Snap_record_{}.mp4",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    ));
    // 尝试多个常见的 ffmpeg 路径（GUI App 启动时 PATH 可能不含 /opt/homebrew/bin）
    let ffmpeg_paths = [
        "/opt/homebrew/bin/ffmpeg",
        "/usr/local/bin/ffmpeg",
        "/opt/local/bin/ffmpeg",
        "/usr/bin/ffmpeg",
        "ffmpeg",
    ];
    let mut success = false;
    let mut last_error = String::new();
    for ff in &ffmpeg_paths {
        let result = std::process::Command::new(ff)
            .args(["-y", "-framerate", "10",
                "-pattern_type", "glob",
                "-i", &dir.join("frame_*.png").to_string_lossy(),
                "-c:v", "libx264", "-pix_fmt", "yuv420p", "-preset", "ultrafast",
            ])
            .arg(output.to_string_lossy().to_string())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|c| c.wait_with_output());
        match result {
            Ok(o) if o.status.success() => { success = true; break; }
            Ok(o) => { last_error = String::from_utf8_lossy(&o.stderr).to_string(); }
            Err(e) => { last_error = format!("{}: {}", ff, e); }
        }
    }
    let _ = fs::remove_dir_all(&dir);
    if success {
        Ok(output.to_string_lossy().to_string())
    } else {
        Err(format!("录制了 {} 帧，但编码失败。\n{}", count, last_error))
    }
}

#[tauri::command]
pub fn get_recording_status() -> serde_json::Value {
    serde_json::json!({
        "recording": IS_RECORDING.load(Ordering::SeqCst),
        "frame_count": FRAME_COUNT.load(Ordering::SeqCst),
    })
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
    ensure_screen_capture_access()?;
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
    ensure_screen_capture_access()?;
    use base64::Engine;
    let primary = primary_monitor()?;
    let img = primary.capture_image().map_err(|e| format!("{}", e))?;
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("{}", e))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());

    // 暂存截图数据，capture.html 加载后通过 get_capture_data 取回
    if let Ok(mut guard) = LAST_CAPTURE_B64.lock() {
        *guard = Some(b64);
    }

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
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .resizable(false)
        .build()
        .map_err(|e| format!("创建截图窗口失败: {}", e))?;

    if let Some(w) = app.get_webview_window("capture") {
        let _ = w.set_size(tauri::LogicalSize::new(width as f64, height as f64));
        let _ = w.set_position(tauri::LogicalPosition::new(0.0, 0.0));
    }
    Ok(())
}

/// 录屏选区：打开覆盖层让用户框选录制区域
#[tauri::command]
pub async fn start_capture_with_record_mode(app: AppHandle) -> Result<(), String> {
    ensure_screen_capture_access()?;
    use base64::Engine;
    let primary = primary_monitor()?;
    let img = primary.capture_image().map_err(|e| format!("{}", e))?;
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("{}", e))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    if let Ok(mut guard) = LAST_CAPTURE_B64.lock() {
        *guard = Some(b64);
    }
    let width = primary.width().map_err(|e| format!("{}", e))?;
    let height = primary.height().map_err(|e| format!("{}", e))?;
    let capture_url = format!("capture.html?w={}&h={}&mode=record", width, height);
    if let Some(w) = app.get_webview_window("capture") {
        w.close().ok();
    }
    let _win = WebviewWindowBuilder::new(&app, "capture", WebviewUrl::App(capture_url.into()))
        .title("")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .resizable(false)
        .build()
        .map_err(|e| format!("创建录屏选区失败: {}", e))?;
    if let Some(w) = app.get_webview_window("capture") {
        let _ = w.set_size(tauri::LogicalSize::new(width as f64, height as f64));
        let _ = w.set_position(tauri::LogicalPosition::new(0.0, 0.0));
    }
    Ok(())
}

/// capture.html 加载后调用此命令取回截图 base64 数据
#[tauri::command]
pub fn get_capture_data() -> Result<String, String> {
    if let Ok(mut guard) = LAST_CAPTURE_B64.lock() {
        if let Some(data) = guard.take() {
            return Ok(data);
        }
    }
    Err("没有可用的截图数据".to_string())
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
    ensure_screen_capture_access()?;
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
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .resizable(false)
        .build()
        .map_err(|e| format!("创建拾色器窗口失败: {}", e))?;

    if let Some(w) = app.get_webview_window("colorpicker") {
        let ww = primary.width().map_err(|e| format!("{}", e))?;
        let wh = primary.height().map_err(|e| format!("{}", e))?;
        let _ = w.set_size(tauri::LogicalSize::new(ww as f64, wh as f64));
        let _ = w.set_position(tauri::LogicalPosition::new(0.0, 0.0));
    }
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
    ensure_screen_capture_access()?;
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
