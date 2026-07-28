import { useEffect, useState } from "react";
import Sidebar from "./components/Sidebar";
import HomePage from "./pages/HomePage";
import ActivatePage from "./pages/ActivatePage";
import SettingsPage from "./pages/SettingsPage";
import HistoryPage from "./pages/HistoryPage";
import { getLicenseStatus, type LicenseStatus } from "./lib/api";
import "./styles/app.css";

export type View = "home" | "history" | "settings" | "activate";

export default function App() {
  const [view, setView] = useState<View>("home");
  const [license, setLicense] = useState<LicenseStatus>({
    activated: false,
    email: null,
    plan: "free",
  });

  // 启动时读取本地授权状态（Rust 端未就绪时忽略错误，走 free 态）
  useEffect(() => {
    getLicenseStatus()
      .then(setLicense)
      .catch(() => {
        /* Rust 后端尚未运行时的降级处理 */
      });
  }, []);

  const pro = license.plan === "pro";

  return (
    <div className="app">
      <Sidebar view={view} onChange={setView} pro={pro} />
      <main className="content">
        {view === "home" && <HomePage pro={pro} onGoActivate={() => setView("activate")} />}
        {view === "history" && <HistoryPage pro={pro} onGoActivate={() => setView("activate")} />}
        {view === "settings" && <SettingsPage license={license} />}
        {view === "activate" && (
          <ActivatePage license={license} onActivated={setLicense} />
        )}
      </main>
    </div>
  );
}
