# Snapin 部署指南

## 架构

```
┌──────────────┐      ┌──────────────┐      ┌─────────────┐
│   浏览器/客户端  │─HTTPS─▶│   Nginx      │─proxy─▶│  Node API   │
│              │      │  (静态+反代)  │      │ (授权+支付) │
└──────────────┘      └──────────────┘      └─────────────┘
                              │                      │
                         官网 HTML              SQLite 文件
                         SSL 证书              (licenses+orders)
```

## 快速部署（Docker Compose）

### 1. 准备服务器
- 1 台云服务器（腾讯云/阿里云，2C4G 足够）
- 域名解析 `snapin.app` → 服务器 IP
- 开放 80/443 端口

### 2. 准备 SSL 证书
```bash
# 方式 A：Let's Encrypt 免费证书
apt install certbot
certbot certonly --standalone -d snapin.app -d www.snapin.app
cp /etc/letsencrypt/live/snapin.app/fullchain.pem deploy/certs/
cp /etc/letsencrypt/live/snapin.app/privkey.pem deploy/certs/

# 方式 B：腾讯云免费证书（推荐，如你已有腾讯云）
# 在腾讯云控制台申请，下载 Nginx 格式证书放入 deploy/certs/
```

### 3. 配置环境变量
```bash
cd deploy
cp .env.example .env
# 编辑 .env 填入支付宝/微信/SMTP 配置
```

### 4. 启动
```bash
docker compose up -d --build
```

### 5. 验证
```bash
curl https://snapin.app              # 官网
curl https://snapin.app/api/license/status?email=test&license_key=test  # API
```

---

## .env 配置项

```env
ADMIN_SECRET=your-secret-here

# SMTP 邮件
SMTP_HOST=smtp.qq.com
SMTP_PORT=465
SMTP_USER=noreply@snapin.app
SMTP_PASS=your-smtp-password

# 支付宝
ALIPAY_APP_ID=2021...
ALIPAY_PRIVATE_KEY=MIIEvg...
ALIPAY_PUBLIC_KEY=MIIBIj...

# 微信支付
WECHAT_APP_ID=wx...
WECHAT_MCH_ID=1234567890
WECHAT_API_KEY_V3=your-v3-key
WECHAT_SERIAL_NO=xxxxx
WECHAT_PRIVATE_KEY=-----BEGIN RSA PRIVATE KEY-----...

# 支付回调
PAYMENT_NOTIFY_URL=https://snapin.app/api/payment/notify
PRICE_CNY=7900
```

---

## 日常运维

```bash
# 查看日志
docker compose logs -f api

# 重启服务
docker compose restart api

# 更新代码后重新部署
git pull && docker compose up -d --build

# 备份数据（SQLite 文件在 volume 中）
docker cp snapin-api:/app/snapin.db ./backup/
```
