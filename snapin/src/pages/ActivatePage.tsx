import { useState } from "react";
import { activateLicense, type LicenseStatus } from "../lib/api";

interface Props {
  license: LicenseStatus;
  onActivated: (s: LicenseStatus) => void;
}

export default function ActivatePage({ license, onActivated }: Props) {
  const [email, setEmail] = useState(license.email || "");
  const [key, setKey] = useState("");
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [loading, setLoading] = useState(false);

  const handleActivate = async () => {
    setLoading(true);
    setMsg(null);
    try {
      const status = await activateLicense(email, key);
      onActivated(status);
      setMsg({ ok: true, text: "激活成功！所有专业版功能已解锁。" });
    } catch (e: unknown) {
      const errMsg = typeof e === "string" ? e : (e as Error)?.message || "激活失败";
      setMsg({ ok: false, text: errMsg });
    } finally {
      setLoading(false);
    }
  };

  if (license.activated) {
    return (
      <div>
        <div className="page-title">授权激活</div>
        <div className="page-sub">当前状态</div>
        <div className="activate-wrap">
          <div className="msg ok">
            ✦ 专业版已激活 · {license.email}
          </div>
          <div className="notice">
            🖥 授权码可激活 2 台设备，可在偏好设置中解绑本机迁移至新设备<br />
            🔒 截图与历史记录全部本地存储，绝不上传云端
          </div>
        </div>
      </div>
    );
  }

  return (
    <div>
      <div className="page-title">激活 Snapin 闪贴</div>
      <div className="page-sub">输入购买时收到的邮箱与授权码，解锁全部专业版功能</div>
      <div className="activate-wrap">
        <div className="head">
          <img src="/snapin.svg" alt="Snapin" />
          <div>
            <div style={{ fontWeight: 700, fontSize: 16 }}>输入授权信息</div>
            <div style={{ fontSize: 12, color: "var(--muted)" }}>一次性买断 · 永久使用</div>
          </div>
        </div>
        <div className="form-label">邮箱</div>
        <input
          className="input"
          placeholder="your@email.com"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
        />
        <div className="form-label">授权码 License Key</div>
        <input
          className="input"
          placeholder="SNPN-XXXX-XXXX-XXXX"
          value={key}
          onChange={(e) => setKey(e.target.value)}
          style={{ fontFamily: "ui-monospace, monospace", letterSpacing: 1 }}
        />
        <div style={{ marginTop: 20, display: "flex", gap: 10 }}>
          <button className="btn-primary" onClick={handleActivate} disabled={loading}>
            {loading ? "验证中…" : "立即激活"}
          </button>
          <button className="btn-ghost">继续试用</button>
        </div>
        {msg && <div className={`msg ${msg.ok ? "ok" : "err"}`}>{msg.text}</div>}
        <div className="notice">
          🔒 截图与历史记录全部本地存储，绝不上传云端<br />
          🖥 一个授权码可激活 2 台设备，可在设置中解绑迁移<br />
          🎁 免费试用版：基础截图 + 标注（不限时、不加水印）
        </div>
      </div>
    </div>
  );
}
