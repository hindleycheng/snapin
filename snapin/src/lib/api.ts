import { invoke } from "@tauri-apps/api/core";

/**
 * 前端调用 Rust 后端命令的封装层。
 * 所有与系统能力（截图、贴图、授权）相关的操作都经由此处，
 * 便于后期替换实现与做 mock。
 */

// ---- 授权 / License ----
export interface LicenseStatus {
  activated: boolean;
  email: string | null;
  plan: "free" | "pro";
}

/** 读取本地授权状态（离线优先，读取本地凭证文件） */
export async function getLicenseStatus(): Promise<LicenseStatus> {
  return invoke<LicenseStatus>("get_license_status");
}

/** 激活：邮箱 + 授权码。首次需联网校验，成功后写入本地凭证。 */
export async function activateLicense(
  email: string,
  licenseKey: string
): Promise<LicenseStatus> {
  return invoke<LicenseStatus>("activate_license", { email, licenseKey });
}

/** 解绑本机（迁移设备时使用） */
export async function deactivateLicense(): Promise<void> {
  return invoke("deactivate_license");
}

// ---- 截图 ----
/** 触发区域截图流程（进入取景模式） */
export async function startCapture(): Promise<void> {
  return invoke("start_capture");
}

/** 全屏截图，返回保存路径 */
export async function captureFullscreen(): Promise<string> {
  return invoke<string>("capture_fullscreen");
}

// ---- 贴图 ----
/** 将剪贴板中的图片贴到屏幕（置顶浮窗） */
export async function pinFromClipboard(): Promise<void> {
  return invoke("pin_from_clipboard");
}

// ---- 拾色器 ----
/** 启动拾色器 */
export async function startColorPicker(): Promise<void> {
  return invoke("start_color_picker");
}

// ---- 历史记录 ----
export interface HistoryEntry {
  id: number;
  filename: string;
  path: string;
  width: number;
  height: number;
  created_at: string;
  has_annotations: boolean;
}

/** 获取全部历史记录（最新在前） */
export async function getHistory(): Promise<HistoryEntry[]> {
  return invoke<HistoryEntry[]>("get_history");
}

/** 删除一条历史记录 */
export async function deleteHistoryEntry(id: number): Promise<void> {
  return invoke("delete_history_entry", { id });
}
