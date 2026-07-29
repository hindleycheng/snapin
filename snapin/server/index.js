/**
 * Snapin 授权服务端
 * 极简 3 接口：生成授权码 / 激活校验 / 解绑设备
 *
 * 数据存储：SQLite（单文件，零运维）
 * 邮件：nodemailer（配置 SMTP 即可）
 *
 * 环境变量（.env 或系统设置）：
 *   PORT=3300
 *   SMTP_HOST / SMTP_PORT / SMTP_USER / SMTP_PASS
 *   ADMIN_SECRET  (用于调用发码接口的简易鉴权)
 */

import express from "express";
import Database from "better-sqlite3";
import { nanoid } from "nanoid";
import nodemailer from "nodemailer";
import { fileURLToPath } from "url";
import fs from "fs";
import { dirname, join } from "path";
import { registerPaymentRoutes } from "./payment.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const PORT = process.env.PORT || 3300;
const ADMIN_SECRET = process.env.ADMIN_SECRET || "snapin-admin-dev";
const MAX_DEVICES = 2;

// ---- 数据库初始化 ----
const db = new Database(join(__dirname, "snapin.db"));
db.pragma("journal_mode = WAL");
db.exec(`
  CREATE TABLE IF NOT EXISTS licenses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    license_key TEXT UNIQUE NOT NULL,
    email TEXT NOT NULL,
    devices_activated INTEGER DEFAULT 0,
    device_ids TEXT DEFAULT '[]',
    created_at TEXT DEFAULT (datetime('now')),
    activated_at TEXT
  );
`);

// ---- 邮件（可选，配了 SMTP 才能发） ----
let transporter = null;
if (process.env.SMTP_HOST) {
  transporter = nodemailer.createTransport({
    host: process.env.SMTP_HOST,
    port: parseInt(process.env.SMTP_PORT || "465"),
    secure: true,
    auth: { user: process.env.SMTP_USER, pass: process.env.SMTP_PASS },
  });
}

// ---- Express 应用 ----
const app = express();
app.use(express.json());

// 静态文件：托管 website/ 目录（官网 + 支付页）
// 优先查找同级 website/ 目录，不存在则��试上级（开发环境）
const websiteDir = fs.existsSync(join(__dirname, "website")) 
  ? join(__dirname, "website")
  : join(__dirname, "..", "website");
app.use(express.static(websiteDir));
// 支付页中的收款码图片（放在 website/assets/ 下）
app.use("/assets", express.static(join(websiteDir, "assets")));

/**
 * POST /api/license/generate
 * 生成一个新授权码并（可选）发送邮件
 * Header: x-admin-secret
 * Body: { email }
 */
app.post("/api/license/generate", (req, res) => {
  if (req.headers["x-admin-secret"] !== ADMIN_SECRET) {
    return res.status(401).json({ error: "Unauthorized" });
  }
  const { email } = req.body;
  if (!email) return res.status(400).json({ error: "email required" });

  // 生成格式：SNPN-XXXX-XXXX-XXXX
  const seg = () => nanoid(4).toUpperCase();
  const key = `SNPN-${seg()}-${seg()}-${seg()}`;

  db.prepare("INSERT INTO licenses (license_key, email) VALUES (?, ?)").run(key, email);

  // 尝试发邮件
  if (transporter) {
    transporter.sendMail({
      from: `"Snapin 闪贴" <${process.env.SMTP_USER}>`,
      to: email,
      subject: "您的 Snapin 授权码",
      html: `<h2>感谢购买 Snapin 闪贴！</h2>
        <p>您的授权码：<code style="font-size:18px;background:#f1f5f9;padding:4px 12px;border-radius:6px">${key}</code></p>
        <p>在软件中输入邮箱和授权码即可激活（最多 ${MAX_DEVICES} 台设备）。</p>
        <p style="color:#888;font-size:12px">此邮件由系统自动发送，请勿直接回复。</p>`,
    }).catch(console.error);
  }

  res.json({ ok: true, license_key: key, email });
});

/**
 * POST /api/license/activate
 * 客户端调用：验证邮箱+授权码，记录设备
 * Body: { email, license_key, device_id }
 */
app.post("/api/license/activate", (req, res) => {
  const { email, license_key, device_id } = req.body;
  if (!email || !license_key || !device_id) {
    return res.status(400).json({ error: "email, license_key, device_id required" });
  }

  const row = db.prepare("SELECT * FROM licenses WHERE license_key = ? AND email = ?").get(license_key, email);
  if (!row) {
    return res.status(403).json({ error: "授权码或邮箱不正确" });
  }

  const devices = JSON.parse(row.device_ids || "[]");
  if (devices.includes(device_id)) {
    // 已激活过的设备，直接通过
    return res.json({ ok: true, plan: "pro" });
  }
  if (devices.length >= MAX_DEVICES) {
    return res.status(403).json({
      error: `已达到最大设备数（${MAX_DEVICES} 台），请先解绑一台设备`,
    });
  }

  devices.push(device_id);
  db.prepare(
    "UPDATE licenses SET device_ids = ?, devices_activated = ?, activated_at = datetime('now') WHERE id = ?"
  ).run(JSON.stringify(devices), devices.length, row.id);

  res.json({ ok: true, plan: "pro" });
});

/**
 * POST /api/license/deactivate
 * 解绑设备
 * Body: { email, license_key, device_id }
 */
app.post("/api/license/deactivate", (req, res) => {
  const { email, license_key, device_id } = req.body;
  if (!email || !license_key || !device_id) {
    return res.status(400).json({ error: "email, license_key, device_id required" });
  }

  const row = db.prepare("SELECT * FROM licenses WHERE license_key = ? AND email = ?").get(license_key, email);
  if (!row) return res.status(403).json({ error: "授权码或邮箱不正确" });

  const devices = JSON.parse(row.device_ids || "[]").filter((d) => d !== device_id);
  db.prepare("UPDATE licenses SET device_ids = ?, devices_activated = ? WHERE id = ?").run(
    JSON.stringify(devices),
    devices.length,
    row.id
  );

  res.json({ ok: true, remaining_devices: devices.length });
});

/**
 * GET /api/license/status?email=&license_key=
 * 查询授权状态（可选，用于用户自查）
 */
app.get("/api/license/status", (req, res) => {
  const { email, license_key } = req.query;
  const row = db.prepare("SELECT * FROM licenses WHERE license_key = ? AND email = ?").get(license_key, email);
  if (!row) return res.status(404).json({ error: "未找到" });
  res.json({
    plan: "pro",
    email: row.email,
    devices_activated: row.devices_activated,
    max_devices: MAX_DEVICES,
    created_at: row.created_at,
  });
});

// 注册支付路由（必须在 listen 之前）
registerPaymentRoutes(app, db, transporter);

app.listen(PORT, () => {
  console.log(`[Snapin Server] 授权服务运行在 http://localhost:${PORT}`);
  console.log(`[Snapin Server] 发码接口: POST /api/license/generate (需 x-admin-secret header)`);
  console.log(`[Snapin Server] 支付接口: POST /api/payment/create`);
});
