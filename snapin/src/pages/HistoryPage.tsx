import { useEffect, useState } from "react";
import { getHistory, deleteHistoryEntry, type HistoryEntry } from "../lib/api";

interface Props {
  pro: boolean;
  onGoActivate: () => void;
}

export default function HistoryPage({ pro, onGoActivate }: Props) {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);

  const loadData = async () => {
    try {
      const data = await getHistory();
      setEntries(data);
    } catch {
      // Rust 后端未运行时降级
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { loadData(); }, []);

  if (!pro) {
    return (
      <div>
        <div className="page-title">历史记录</div>
        <div className="page-sub">本地截图库（需激活专业版）</div>
        <div className="empty">
          <div style={{ fontSize: 40, marginBottom: 12 }}>🔒</div>
          <div>历史记录为专业版功能</div>
          <button className="btn-primary" style={{ marginTop: 16 }} onClick={onGoActivate}>
            去激活
          </button>
        </div>
      </div>
    );
  }

  const handleDelete = async (id: number) => {
    await deleteHistoryEntry(id);
    setEntries((prev) => prev.filter((e) => e.id !== id));
  };

  return (
    <div>
      <div className="page-title">历史记录</div>
      <div className="page-sub">本地截图库 · 全部存储在本机，绝不上传 · 共 {entries.length} 条</div>
      {loading ? (
        <div className="empty">加载中...</div>
      ) : entries.length === 0 ? (
        <div className="empty">暂无截图记录<br />截一张试试？按 ⌘⇧A</div>
      ) : (
        <div className="hgrid">
          {entries.map((e) => (
            <div className="hcard" key={e.id}>
              <div className="thumb" style={{ position: "relative" }}>
                {e.has_annotations && (
                  <span style={{
                    position: "absolute", top: 6, right: 6, background: "var(--brand)",
                    color: "#fff", fontSize: 10, padding: "2px 6px", borderRadius: 4
                  }}>
                    有标注
                  </span>
                )}
              </div>
              <div className="meta">
                <b>{e.filename}</b>
                {e.width}×{e.height} · {e.created_at}
                <span
                  onClick={() => handleDelete(e.id)}
                  style={{ float: "right", cursor: "pointer", color: "var(--danger)", fontSize: 12 }}
                >
                  删除
                </span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
