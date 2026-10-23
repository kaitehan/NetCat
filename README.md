# NetCat（奈特猫）

> 本地守候每一次请求

NetCat（奈特猫）是一款面向开发者的本地 HTTP/HTTPS 抓包与 Mock 工具，可查看请求与响应、筛选流量、重放请求、构造 Mock，并管理本地 CA 与系统代理。流量、规则与设置默认只保存在本机。

当前版本面向 macOS，界面使用 Vue 3，桌面壳与代理运行时使用 Tauri 2 和 Rust。应用由 kaite han 开发。

> 仅可捕获您拥有或已明确获准测试的设备、应用和网络。不要将本工具用于未授权的流量拦截、凭据收集或隐私数据访问。

## MVP 能力

- 本机 HTTP/1.1 与 HTTPS CONNECT 代理，默认监听 `127.0.0.1:28086`
- 用户主动开启的局域网模式，仅接受私有网络来源
- HTTPS 动态叶子证书与本地根 CA；CA 私钥仅保存在 macOS 钥匙串
- 精确、包含、正则 URL Mock，支持方法、优先级、状态码、Header、正文和延迟
- SQLite 持久化、5000 条滚动上限、单向正文 10 MB 截断
- 关键词、方法、状态码和域名过滤，Request/Response/Timing 详情
- HAR 1.2 导出和可执行 cURL 复制
- 当前 macOS 网络服务的 HTTP/HTTPS 系统代理设置与恢复

当前版本不解析 HTTP/2、QUIC、WebSocket 帧或非 HTTP TCP。开启证书固定的应用可能无法被解密。

## 开发环境

- macOS 12 或更高版本
- Apple Silicon Mac
- Node.js 22 或更高版本
- Rust 1.85 或更高版本
- Xcode Command Line Tools

```bash
npm install
npm run dev
```

浏览器预览运行在 `http://127.0.0.1:1420`。运行真实桌面应用：

```bash
npm run tauri dev
```

运行测试和构建：

```bash
npm test
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
npm run tauri build
```

60 秒压力验收默认标记为 ignored，单独执行：

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test proxy_stress sustains_200_qps_for_60_seconds_and_keeps_5000_rows -- --ignored --exact
```

## 首次使用

1. 阅读并确认授权使用声明。
2. 打开“证书”，点击“生成证书”。公开证书写入应用数据目录，私钥写入 macOS 钥匙串。
3. 导出公开 CA，双击加入“钥匙串访问”。在“系统”钥匙串中找到 `NetCat Local Root CA`，展开“信任”，将“使用此证书时”设为“始终信任”。
4. 启动代理。需要让遵循 macOS 系统代理的应用自动接入时，再开启“系统代理”。
5. 完成测试后停止代理。应用会恢复修改前保存的 HTTP 与 HTTPS 代理配置。

证书只应用于受控开发环境。不要导入生产设备，也不要分发给无关人员。

## 从 NetCapture 升级

- 升级前请停止代理并完全退出旧版 NetCapture，避免两个版本同时修改系统代理。
- 首次使用 NetCat 启动时，如果新数据库尚不存在，应用会将旧目录 `~/Library/Application Support/com.netcapture.desktop/netcapture.sqlite3` 中的流量、Mock 规则和设置复制到新数据库。
- 旧目录、旧数据库和旧钥匙串记录不会被删除，便于回滚；已有 NetCat 数据不会被覆盖。
- 升级后会生成新的 `NetCat Local Root CA`。请重新导入并信任新证书，确认 HTTPS 抓包正常后，可从“钥匙串访问”手动删除旧的 `NetCapture Local Root CA`。
- 证书下载地址已切换为 `/__netcat/ca.crt`，HAR 扩展字段已切换为 `_netcat_*`，不再兼容旧名称。

## 手机接入

1. Mac 与手机连接同一可信 Wi-Fi。
2. 在 NetCat 中开启“局域网”并启动代理。
3. 将手机 Wi-Fi 的 HTTP 代理设为界面显示的 Mac IP 和端口。
4. 扫描证书配置中的二维码，或访问 `http://<Mac-IP>:28086/__netcat/ca.crt`。
5. iOS 安装描述文件后，还需进入“设置 > 通用 > 关于本机 > 证书信任设置”开启完全信任。
6. Android 7+ 应用默认可能不信任用户 CA；被测应用需在 Network Security Config 中明确允许。

## 故障恢复

### 手动关闭系统代理

若应用异常退出且自动恢复失败，先查看网络服务名称：

```bash
networksetup -listallnetworkservices
```

然后对实际使用的服务执行，例如 Wi-Fi：

```bash
networksetup -setwebproxystate "Wi-Fi" off
networksetup -setsecurewebproxystate "Wi-Fi" off
```

如果修改前使用了自定义代理，应在“系统设置 > 网络 > 当前网络 > 详细信息 > 代理”中恢复原服务器、端口和启用状态，而不是简单关闭。

### 常见错误

- `端口不可用`：更换端口，或关闭占用该端口的程序。
- `CA 未生成`：打开证书面板并生成本地 CA。
- HTTPS 握手失败：确认 CA 已信任；目标应用也可能启用了证书固定。
- 上游连接失败或超时：确认目标服务器、DNS 和当前网络可访问。
- 系统代理恢复失败：按上面的 `networksetup` 步骤恢复，并检查当前网络服务名称是否已变化。

## 卸载与删除 CA

1. 停止代理并关闭系统代理。
2. 在“钥匙串访问”中搜索并删除 `NetCat Local Root CA`。
3. 删除“我的证书”或“证书”列表中同名的残留项。
4. 如需删除钥匙串中的 CA 私钥记录，可执行：

```bash
security delete-generic-password -s com.kaitehan.netcat -a root-ca-private-key
```

5. 删除应用数据目录 `~/Library/Application Support/com.kaitehan.netcat`，再将 `NetCat.app` 移到废纸篓。

删除应用数据会永久移除本地流量、Mock 规则和设置。HAR 或 CA 公钥导出文件不在该目录时需单独处理。

## 数据与安全边界

- 无账号、云同步、遥测和远程上传。
- CA 私钥不会进入 SQLite、日志、HAR、cURL 或证书导出文件。
- 暂停捕获只停止创建新展示记录，代理转发仍继续。
- 非法正则、Header、状态码、JSON 或延迟不会保存为运行时规则。
- TLS 失败只提示“证书未信任或应用可能启用证书固定”，不会声称已识别具体原因。

## 下一步规划

- Windows 兼容：系统代理管理、Windows 证书存储、安装包和平台兼容性测试。
- HTTP/2 完整解析、WebSocket 帧级调试，以及 QUIC / HTTP/3 支持。
- SSL Pinning 绕过、JavaScript 脚本系统和弱网模拟。
- 自动更新、macOS 签名与公证，以及公开分发所需的合规工作。
- 基础断点改包仍受性能和稳定性门禁约束；若未通过 0.3.0 门禁，则作为后续版本候选，不视为 0.3.0 已交付能力。
