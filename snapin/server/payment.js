/**
 * Snapin 支付模块（个人收款码版）
 *
 * 流程：
 * 1. 管理员预生成一批授权码（库存）
 * 2. 买家提交邮箱 → 生成订单 → 显示个人收款码（支付宝/微信）
 * 3. 买家转账时备注订单号
 * 4. 管理员在后台确认收款 → 自动分配授权码 → 发邮件
 *    或：独角数卡 webhook 回调自动确认
 *
 * 环境变量：
 *   ADMIN_SECRET        - 管理接口鉴权
 *   ALIPAY_QR_URL       - 个人支付宝收款码图片 URL
 *   WECHAT_QR_URL       - 个人微信收款码图片 URL
 *   PRICE_DISPLAY       - 显示价格（如 "79"）
 *   SMTP_HOST/PORT/USER/PASS - 邮件配置
 */

import { nanoid } from "nanoid";

const PRICE_DISPLAY = process.env.PRICE_DISPLAY || "79";
const ADMIN_SECRET = process.env.ADMIN_SECRET || "snapin-admin-dev";

/**
 * 注册支付路由
 */
export function registerPaymentRoutes(app, db, transporter) {
  // 创建表
  db.exec(`
    CREATE TABLE IF NOT EXISTS orders (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      order_no TEXT UNIQUE NOT NULL,
      email TEXT NOT NULL,
      channel TEXT NOT NULL,
      status TEXT DEFAULT 'pending',
      license_key TEXT,
      created_at TEXT DEFAULT (datetime('now')),
      confirmed_at TEXT
    );
    CREATE TABLE IF NOT EXISTS license_pool (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      license_key TEXT UNIQUE NOT NULL,
      status TEXT DEFAULT 'available',
      assigned_to TEXT,
      created_at TEXT DEFAULT (datetime('now'))
    );
  `);

  // ============================================================
  // POST /api/payment/create  买家下单
  // Body: { email, channel: "alipay" | "wechat" }
  // ============================================================
  app.post("/api/payment/create", (req, res) => {
    const { email, channel } = req.body;
    if (!email || !email.includes("@")) return res.status(400).json({ error: "请输入有效邮箱" });
    if (!["alipay", "wechat"].includes(channel)) return res.status(400).json({ error: "请选择支付方式" });

    // 检查库存
    const available = db.prepare("SELECT COUNT(*) as count FROM license_pool WHERE status='available'").get();
    if (available.count === 0) return res.status(503).json({ error: "授权码暂时售罄，请稍后再试" });

    const orderNo = "SN" + Date.now().toString(36).toUpperCase() + nanoid(3).toUpperCase();

    db.prepare("INSERT INTO orders (order_no, email, channel) VALUES (?,?,?)").run(orderNo, email, channel);

    // 返回收款码信息
    const qrUrl = channel === "alipay"
      ? (process.env.ALIPAY_QR_URL || "/assets/alipay-qr.png")
      : (process.env.WECHAT_QR_URL || "/assets/wechat-qr.png");

    res.json({
      ok: true,
      order_no: orderNo,
      qr_url: qrUrl,
      price: PRICE_DISPLAY,
      channel,
      message: `请转账 ¥${PRICE_DISPLAY}，备注「${orderNo}」`,
    });
  });

  // ============================================================
  // GET /api/payment/status?order_no=XXX  买家轮询订单状态
  // ============================================================
  app.get("/api/payment/status", (req, res) => {
    const { order_no } = req.query;
    if (!order_no) return res.status(400).json({ error: "缺少 order_no" });
    const row = db.prepare("SELECT * FROM orders WHERE order_no = ?").get(order_no);
    if (!row) return res.status(404).json({ error: "订单不存在" });
    res.json({
      order_no: row.order_no,
      status: row.status,
      license_key: row.status === "paid" ? row.license_key : null,
    });
  });

  // ============================================================
  // POST /api/admin/confirm  管理员确认收款
  // Header: x-admin-secret
  // Body: { order_no }
  // ============================================================
  app.post("/api/admin/confirm", (req, res) => {
    if (req.headers["x-admin-secret"] !== ADMIN_SECRET) {
      return res.status(401).json({ error: "Unauthorized" });
    }
    const { order_no } = req.body;
    if (!order_no) return res.status(400).json({ error: "缺少 order_no" });

    const result = fulfillOrder(db, order_no, transporter);
    if (result.error) return res.status(400).json(result);
    res.json(result);
  });

  // ============================================================
  // POST /api/admin/generate-keys  批量生成授权码入库
  // Header: x-admin-secret
  // Body: { count: 10 }
  // ============================================================
  app.post("/api/admin/generate-keys", (req, res) => {
    if (req.headers["x-admin-secret"] !== ADMIN_SECRET) {
      return res.status(401).json({ error: "Unauthorized" });
    }
    const count = Math.min(parseInt(req.body.count) || 10, 100);
    const keys = [];
    const stmt = db.prepare("INSERT INTO license_pool (license_key) VALUES (?)");
    for (let i = 0; i < count; i++) {
      const seg = () => nanoid(4).toUpperCase();
      const key = `SNPN-${seg()}-${seg()}-${seg()}`;
      stmt.run(key);
      keys.push(key);
    }
    const total = db.prepare("SELECT COUNT(*) as c FROM license_pool WHERE status='available'").get();
    res.json({ ok: true, generated: keys.length, total_available: total.c, keys });
  });

  // ============================================================
  // GET /api/admin/orders  查看所有订单
  // Header: x-admin-secret
  // ============================================================
  app.get("/api/admin/orders", (req, res) => {
    if (req.headers["x-admin-secret"] !== ADMIN_SECRET) {
      return res.status(401).json({ error: "Unauthorized" });
    }
    const status = req.query.status || "all";
    const rows = status === "all"
      ? db.prepare("SELECT * FROM orders ORDER BY id DESC LIMIT 100").all()
      : db.prepare("SELECT * FROM orders WHERE status=? ORDER BY id DESC LIMIT 100").all(status);
    res.json({ orders: rows });
  });

  // ============================================================
  // GET /api/admin/pool  查看授权码库存
  // ============================================================
  app.get("/api/admin/pool", (req, res) => {
    if (req.headers["x-admin-secret"] !== ADMIN_SECRET) {
      return res.status(401).json({ error: "Unauthorized" });
    }
    const available = db.prepare("SELECT COUNT(*) as c FROM license_pool WHERE status='available'").get();
    const assigned = db.prepare("SELECT COUNT(*) as c FROM license_pool WHERE status='assigned'").get();
    res.json({ available: available.c, assigned: assigned.c });
  });

  // ============================================================
  // POST /api/webhook/dujiaoka  独角数卡回调（可选对接）
  // ============================================================
  app.post("/api/webhook/dujiaoka", (req, res) => {
    // 独角数卡支付成功后会 POST 通知
    const { out_order_id, status } = req.body;
    if (status === "1" && out_order_id) {
      fulfillOrder(db, out_order_id, transporter);
    }
    res.send("ok");
  });
}

// ============================================================
// 履约：分配授权码 + 发邮件
// ============================================================
function fulfillOrder(db, orderNo, transporter) {
  const order = db.prepare("SELECT * FROM orders WHERE order_no = ? AND status = 'pending'").get(orderNo);
  if (!order) return { error: "订单不存在或已处理" };

  // 从池中取一个可用的授权码
  const key = db.prepare("SELECT * FROM license_pool WHERE status='available' LIMIT 1").get();
  if (!key) return { error: "授权码库存不足，请先生成" };

  // 标记为已分配
  db.prepare("UPDATE license_pool SET status='assigned', assigned_to=? WHERE id=?").run(order.email, key.id);
  // 更新订单
  db.prepare("UPDATE orders SET status='paid', license_key=?, confirmed_at=datetime('now') WHERE order_no=?").run(key.license_key, orderNo);
  // 写入 licenses 表
  db.prepare("INSERT OR IGNORE INTO licenses (license_key, email) VALUES (?,?)").run(key.license_key, order.email);

  // 发邮件
  if (transporter) {
    transporter.sendMail({
      from: `"Snapin 闪贴" <${process.env.SMTP_USER}>`,
      to: order.email,
      subject: "您的 Snapin 授权码",
      html: `<div style="font-family:system-ui;max-width:500px;margin:0 auto">
        <h2 style="color:#3b82f6">🎉 感谢购买 Snapin 闪贴！</h2>
        <p>您的授权码：</p>
        <div style="font-size:22px;font-weight:bold;font-family:monospace;background:#f1f5f9;padding:14px 20px;border-radius:10px;letter-spacing:1px;text-align:center">${key.license_key}</div>
        <p style="margin-top:16px">在 Snapin 软件中输入邮箱 + 授权码即可激活（最多 2 台设备）。</p>
        <p style="color:#888;font-size:12px;margin-top:20px;border-top:1px solid #eee;padding-top:12px">订单号：${orderNo}<br>此邮件由系统自动发送，请勿直接回复。如有问题请联系 support@snapin.app</p>
      </div>`,
    }).catch(e => console.error("[Mail Error]", e));
  }

  console.log(`[Payment] ✓ 订单 ${orderNo} 已确认 → ${key.license_key} → ${order.email}`);
  return { ok: true, license_key: key.license_key, email: order.email };
}
