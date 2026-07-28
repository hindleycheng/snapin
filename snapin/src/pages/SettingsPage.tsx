import type { LicenseStatus } from "../lib/api";

interface Props {
  license: LicenseStatus;
}

export default function SettingsPage({ license }: Props) {
  return (
    <div>
      <div className="page-title">偏好设置</div>
      <div className="page-sub">自定义快捷键、保存行为和通用选项</div>

      <h3 style={{ fontSize: 15, marginBottom: 12 }}>快捷键</h3>
      <div className="field">
        <div><div className="lb">区域截图</div><div className="hint">按下即进入取景模式</div></div>
        <span className="kbd-tag">⌘ ⇧ A / Ctrl ⇧ A</span>
      </div>
      <div className="field">
        <div><div className="lb">全屏截图</div><div className="hint">直接捕捉当前屏幕</div></div>
        <span className="kbd-tag">⌘ ⇧ F</span>
      </div>
      <div className="field">
        <div><div className="lb">贴图</div><div className="hint">把剪贴板图片置顶贴在屏幕上</div></div>
        <span className="kbd-tag">F3</span>
      </div>
      <div className="field">
        <div><div className="lb">拾色器</div></div>
        <span className="kbd-tag">⌘ ⇧ C</span>
      </div>

      <h3 style={{ fontSize: 15, margin: "26px 0 12px" }}>保存与输出</h3>
      <div className="field">
        <div><div className="lb">默认格式</div></div>
        <span className="kbd-tag">PNG</span>
      </div>
      <div className="field">
        <div><div className="lb">文件名模板</div><div className="hint">支持日期时间变量</div></div>
        <span className="kbd-tag">Snap_&#123;yyyyMMdd_HHmmss&#125;</span>
      </div>

      <h3 style={{ fontSize: 15, margin: "26px 0 12px" }}>授权</h3>
      <div className="field">
        <div>
          <div className="lb">当前状态</div>
          <div className="hint">{license.activated ? `已激活 · ${license.email}` : "免费版"}</div>
        </div>
        <span className="kbd-tag">{license.plan === "pro" ? "专业版" : "Free"}</span>
      </div>
    </div>
  );
}
