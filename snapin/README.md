# Snapin 闪贴

跨平台截图与贴图工具 · macOS / Windows · Tauri 2 + React + TypeScript

## 技术栈

| 层 | 技术 |
|---|------|
| 前端 | React 18 + TypeScript + Vite |
| 后端 | Rust + Tauri 2 |
| 截屏 | 原生 API（待接入 xcap crate） |
| 授权 | 邮箱 + 授权码，本地凭证离线校验 |

## 环境要求

- **Node.js** >= 18
- **Rust** >= 1.77（含 cargo）
- **系统依赖**：
  - macOS：Xcode Command Line Tools
  - Windows：Visual Studio Build Tools + WebView2

## 安装 Rust（如果还没装）

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustc --version  # 确认安装成功
```

## 快速开始

```bash
cd snapin

# 安装前端依赖
npm install

# 开发模式（同时启动 Vite 前端 + Rust 后端）
npm run tauri dev

# 构建发布包
npm run tauri build
```

## 目录结构

```
snapin/
├── index.html              # 入口 HTML
├── package.json            # 前端依赖与脚本
├── vite.config.ts          # Vite 配置
├── tsconfig.json
├── public/
│   └── snapin.svg          # Logo SVG
├── src/                    # 前端源码
│   ├── main.tsx            # React 入口
│   ├── App.tsx             # 根组件（路由）
│   ├── components/
│   │   └── Sidebar.tsx     # 侧边栏导航
│   ├── pages/
│   │   ├── HomePage.tsx    # 主页（功能卡片）
│   │   ├── ActivatePage.tsx# 授权激活
│   │   ├── HistoryPage.tsx # 历史记录（本地）
│   │   └── SettingsPage.tsx# 偏好设置
│   ├── lib/
│   │   └── api.ts          # Tauri 命令封装层
│   └── styles/
│       ├── global.css      # 全局主题变量
│       └── app.css         # 组件样式
└── src-tauri/              # Rust 后端
    ├── Cargo.toml          # Rust 依赖
    ├── build.rs
    ├── tauri.conf.json     # Tauri 应用配置
    ├── src/
    │   ├── main.rs         # 应用入口
    │   └── lib.rs          # 命令实现（授权/截图/贴图）
    └── icons/              # 应用图标（待放入）
```

## 当前状态

- [x] 项目脚手架搭建
- [x] 前端 UI 骨架（主页/激活/设置/历史）
- [x] 授权激活流程（本地凭证读写）
- [ ] 截图核心：区域选取 + 全屏截图
- [ ] 标注编辑器
- [ ] 贴图钉屏（置顶浮窗）
- [ ] 全局快捷键注册
- [ ] 系统托盘
- [ ] 拾色器
- [ ] 历史记录本地存储（SQLite）
- [ ] 构建签名与自动更新

## License

Proprietary · 一次性买断制
