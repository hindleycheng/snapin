import type { View } from "../App";

interface Props {
  view: View;
  onChange: (v: View) => void;
  pro: boolean;
}

const items: { key: View; label: string; icon: JSX.Element }[] = [
  {
    key: "home",
    label: "主页",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
        <path d="M3 10l9-7 9 7v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
      </svg>
    ),
  },
  {
    key: "history",
    label: "历史记录",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
        <circle cx="12" cy="12" r="9" />
        <path d="M12 8v4l3 2" />
      </svg>
    ),
  },
  {
    key: "settings",
    label: "偏好设置",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
        <circle cx="12" cy="12" r="3" />
        <path d="M19 12a7 7 0 0 0-.1-1l2-1.5-2-3.5-2.4 1a7 7 0 0 0-1.7-1L14.5 2h-5l-.3 2.5a7 7 0 0 0-1.7 1l-2.4-1-2 3.5L3 11a7 7 0 0 0 0 2l-2 1.5 2 3.5 2.4-1a7 7 0 0 0 1.7 1l.3 2.5h5l.3-2.5a7 7 0 0 0 1.7-1l2.4 1 2-3.5-2-1.5a7 7 0 0 0 .1-1z" />
      </svg>
    ),
  },
  {
    key: "activate",
    label: "授权激活",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
        <rect x="3" y="11" width="18" height="10" rx="2" />
        <path d="M7 11V7a5 5 0 0 1 10 0v4" />
      </svg>
    ),
  },
];

export default function Sidebar({ view, onChange, pro }: Props) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <img src="/snapin.svg" alt="Snapin" />
        <div className="name">
          Snap<span className="in">in</span>
        </div>
      </div>
      {items.map((it) => (
        <div
          key={it.key}
          className={"nav-item" + (view === it.key ? " active" : "")}
          onClick={() => onChange(it.key)}
        >
          {it.icon}
          {it.label}
        </div>
      ))}
      <div className="spacer" />
      <div className={"plan-badge" + (pro ? " pro" : "")}>
        {pro ? "✦ 专业版已激活" : "免费版 · 点击激活解锁全部功能"}
      </div>
    </aside>
  );
}
