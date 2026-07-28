# Snapin 签名与公证配置指南

## macOS 代码签名 + 公证

### 前置条件
1. Apple Developer Account（$99/年）
2. 在 Apple Developer 中创建「Developer ID Application」证书
3. 创建 App-specific Password（用于公证 notarytool）

### 环境变量（GitHub Secrets）
```
APPLE_CERTIFICATE        - Base64 编码的 .p12 证书文件
APPLE_CERTIFICATE_PASSWORD - .p12 密码
APPLE_SIGNING_IDENTITY   - "Developer ID Application: Your Name (TEAMID)"
APPLE_ID                 - Apple ID 邮箱
APPLE_PASSWORD           - App-specific password
APPLE_TEAM_ID            - 10 位 Team ID
```

### 本地测试签名
```bash
# 导出证书
security find-identity -v -p codesigning

# 手动签名（Tauri build 会自动处理）
codesign --deep --force --sign "Developer ID Application: XXX" target/release/bundle/macos/Snapin.app

# 手动公证
xcrun notarytool submit Snapin.dmg --apple-id YOUR_ID --password YOUR_APP_PASSWORD --team-id YOUR_TEAM
xcrun stapler staple Snapin.dmg
```

### tauri.conf.json 配置
`bundle.macOS.signingIdentity` 设为证书名，或通过环境变量传入（CI 推荐后者）。

---

## Windows 代码签名

### 前置条件
1. 代码签名证书（EV 或 OV），推荐 DigiCert / Sectigo / GlobalSign
2. 证书导入到 Windows 证书存储或 .pfx 文件

### 环境变量
```
WINDOWS_CERTIFICATE       - Base64 编码的 .pfx
WINDOWS_CERTIFICATE_PASSWORD - .pfx 密码
```

### tauri.conf.json 配置
```json
"bundle": {
  "windows": {
    "certificateThumbprint": "YOUR_THUMBPRINT",
    "digestAlgorithm": "sha256",
    "timestampUrl": "http://timestamp.digicert.com"
  }
}
```

### 本地测试签名
```powershell
signtool sign /f cert.pfx /p PASSWORD /tr http://timestamp.digicert.com /td sha256 Snapin.exe
```

---

## Tauri Updater 签名

Updater 使用 Ed25519 密钥对，与代码签名独立。

```bash
# 生成密钥对（一次性）
npx @tauri-apps/cli signer generate -w ~/.tauri/snapin.key

# 输出：
# Private key saved to ~/.tauri/snapin.key
# Public key: dW50cnVz...（放入 tauri.conf.json plugins.updater.pubkey）
```

GitHub Secrets:
```
TAURI_SIGNING_PRIVATE_KEY          - 私钥内容
TAURI_SIGNING_PRIVATE_KEY_PASSWORD - 私钥密码（如果设了的话）
```

---

## CI 流程总结

1. Push tag `v0.1.0` → 触发 `.github/workflows/release.yml`
2. macOS: 编译 aarch64 + x86_64 → 签名 + 公证 → 输出 .dmg + .app.tar.gz + .sig
3. Windows: 编译 x86_64 → 签名 → 输出 .exe/.msi + .sig
4. 上传 artifacts → 发布到 GitHub Releases 或自建 CDN
5. 更新 `server/update-manifest.json` 的 version / url / signature
