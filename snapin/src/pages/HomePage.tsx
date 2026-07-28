import {
  startCapture,
  captureFullscreen,
  pinFromClipboard,
  startColorPicker,
} from "../lib/api";

interface Props {
  pro: boolean;
  onGoActivate: () => void;
}

/** 免费可用 / 需激活的功能卡片配置 */
const actions = [
  {
    title: "区域截图",
    desc: "拖拽框选或智能识别窗口",
    kbd: "⌘⇧A / Ctrl⇧A",
    pro: false,
    run: () => startCapture(),
    icon: (
      <svg viewBox="0 0 24 24" fill="none" strokeWidth="2">
        <path d="M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3" />
      </svg>
    ),
  },
  {
    title: "全屏截图",
    desc: "捕捉整个屏幕",
    kbd: "⌘⇧F",
    pro: false,
    run: () => captureFullscreen(),
    icon: (
      <svg viewBox="0 0 24 24" fill="none" strokeWidth="2">
        <rect x="3" y="4" width="18" height="14" rx="2" />
        <path d="M8 21h8" />
      </svg>
    ),
  },
  {
    title: "贴图钉屏",
    desc: "把剪贴板图片置顶贴在屏幕上",
    kbd: "F3",
    pro: true,
    run: () => pinFromClipboard(),
    icon: (
      <svg viewBox="0 0 24 24" fill="none" strokeWidth="2">
        <path d="M15 4v7h5l-8 9-8-9h5V4z" />
      </svg>
    ),
  },
  {
    title: "拾色器",
    desc: "屏幕取色，输出 HEX / RGB",
    kbd: "⌘⇧C",
    pro: true,
    run: () => startColorPicker(),
    icon: (
      <svg viewBox="0 0 24 24" fill="none" strokeWidth="2">
        <path d="M12 2a4 4 0 0 1 4 4c0 4-4 6-4 12-0-6-4-8-4-12a4 4 0 0 1 4-4z" />
      </svg>
    ),
  },
];

export default function HomePage({ pro, onGoActivate }: Props) {
  const handle = async (a: (typeof actions)[number]) => {
    if (a.pro && !pro) {
      onGoActivate();
      return;
    }
    try {
      await a.run();
    } catch {
      // Rust 后端未运行时的占位提示
      alert(`「${a.title}」需要在 Tauri 环境下运行（npm run tauri dev）`);
    }
  };

  return (
    <div>
      <div className="page-title">Snapin 闪贴</div>
      <div className="page-sub">截了就贴，贴了就用 · 选择一个操作，或使用全局快捷键</div>
      <div className="card-grid">
        {actions.map((a) => (
          <div key={a.title} className="action-card" onClick={() => handle(a)}>
            {a.pro && !pro && <span className="lock">需激活</span>}
            <div className="ic">{a.icon}</div>
            <h3>{a.title}</h3>
            <p>{a.desc}</p>
            <span className="kbd">{a.kbd}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
