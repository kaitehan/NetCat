# NetCat（奈特猫）

[简体中文](README.md) | [English](README.en.md)

> 本地守候每一次请求。

NetCat（奈特猫）是一款面向开发者的本地 HTTP/HTTPS 调试代理。它提供流量捕获与筛选、请求编辑重放、Mock 规则、JavaScript 脚本、代理层弱网模拟、经授权的 SSL Pinning 调试、HAR 导出、系统代理和本地 CA 管理。

NetCat 基于 Vue 3、Tauri 2 和 Rust 构建。请求记录、Mock 规则和应用设置默认保存在本机，不依赖账号、云服务、遥测或远程数据上传。

> A local-first HTTP/HTTPS debugging proxy with traffic inspection, request replay, mocking, system proxy integration, and local CA management.

NetCat 与传统的 `netcat` / `nc` 命令行网络工具没有关联。本项目聚焦桌面端 HTTP/HTTPS 接口调试。

> 仅可捕获您拥有或已明确获准测试的设备、应用和网络。不要将本工具用于未授权的流量拦截、凭据收集或隐私数据访问。

## 核心能力

- 本机 HTTP/1.1 与 HTTPS CONNECT 代理，默认监听 `127.0.0.1:28086`
- 互斥的电脑/手机抓包模式；手机模式只接受扫码配对的私有网络设备，不抓取电脑请求
- HTTPS 动态叶子证书与本地根 CA；macOS 私钥保存在钥匙串，Windows 私钥使用当前用户 DPAPI 加密保存
- 请求编辑与重放，支持 GET、POST、JSON、表单和二进制正文
- 精确、包含、正则 URL Mock，支持方法、优先级、状态码、Header、正文、延迟、匹配测试和命中统计
- SQLite 持久化、5000 条滚动上限、单向正文 10 MB 截断
- 本机与多手机设备 Tab，以及关键词、方法、状态码和域名组合过滤
- HAR 1.2 导出和可执行 cURL 复制
- macOS 当前网络服务、Windows 当前用户的 HTTP/HTTPS 系统代理设置与恢复
- 嵌入式 QuickJS 请求/响应脚本，支持改写 Header、URL、正文或直接生成响应
- 高速 4G、低速 4G、3G、离线和自定义代理层弱网配置，切换即时生效
- 外部 Frida 驱动的 Android/iOS SSL Pinning 调试工作区，支持 USB、Gadget、Spawn 和 Attach

当前版本不解析 HTTP/2、QUIC、WebSocket 帧或非 HTTP TCP。Pinning 注入在 Windows 和移动端仍标记为实验性。

## 技术栈

- Vue 3、TypeScript、Vite 和 Monaco Editor
- Tauri 2 与 Rust
- SQLite 本地持久化
- Vitest、Vue Testing Library 与 Rust 测试套件

## 平台状态

| 平台 | 状态 | 说明 |
| --- | --- | --- |
| macOS 12+ | 主要支持 | 已实现系统代理、钥匙串私钥以及 `.app` / DMG 打包 |
| Windows 10 22H2 / Windows 11 | 实验性支持 | 仅支持 x86_64 MSVC；已实现当前用户代理、DPAPI 私钥、证书信任与 NSIS 构建配置，仍需完成 Windows 实机验收 |

项目当前处于内部开发阶段，不建议直接用于生产环境或未受控网络。

## 开发环境

- macOS 12+（Apple Silicon Mac）或 Windows 10 22H2 / Windows 11（x86_64）
- Node.js 22 或更高版本
- Rust 1.85 或更高版本
- macOS：Xcode Command Line Tools
- Windows：Visual Studio 2022 Build Tools 的“使用 C++ 的桌面开发”工作负载（含 Windows SDK）、Rust `x86_64-pc-windows-msvc` 工具链和 Microsoft Edge WebView2 Runtime

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

Windows NSIS 安装包需在 Windows 开发环境构建：

```powershell
rustup target add x86_64-pc-windows-msvc
npm ci
npm run tauri:build:windows
```

安装包输出到 `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`。Windows CI 使用 `windows-2022` 依次执行依赖安装、前端测试、构建体积检查、Rust 测试和真实 NSIS 构建，再上传 `.exe` artifact。macOS 保持 `npm run tauri -- build --bundles app,dmg` 的 `.app` / DMG 打包方式。

Windows 安装包按当前用户安装。运行应用需要 WebView2；NSIS 在缺少运行时时使用 `downloadBootstrapper` 联网下载并安装，因此首次安装需要可访问微软下载服务，受限网络环境请预先安装 WebView2 Evergreen Runtime。当前 macOS 开发主机未生成或运行 Windows 安装包；Windows 构建结果以 CI 为准，Windows 10 22H2 / 11 的安装、运行与卸载仍需在对应系统验收。

60 秒压力验收默认标记为 ignored，单独执行：

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test proxy_stress sustains_200_qps_for_60_seconds_and_keeps_5000_rows -- --ignored --exact
```

## 首次使用

1. 阅读并确认授权使用声明。
2. 打开“证书”，点击“生成证书”。公开证书写入应用数据目录；macOS 私钥写入钥匙串，Windows 私钥使用当前用户 DPAPI 加密保存到 `ca-private-key.dpapi`，不能仅复制该文件到其他账号使用。
3. macOS：导出公开 CA，双击加入“钥匙串访问”。在“系统”钥匙串中找到 `NetCat Local Root CA`，展开“信任”，将“使用此证书时”设为“始终信任”。Windows：在证书面板安装并信任 CA，授权后仅写入当前用户的“受信任的根证书颁发机构”；也可导出 CA，在证书导入向导选择“当前用户”及该存储区，并核对指纹。
4. 启动代理。需要让遵循系统代理的应用自动接入时，再开启“系统代理”。Windows 只修改当前用户 Internet Settings 的手动 HTTP/HTTPS 代理，保留绕过列表、PAC 和自动检测设置；不会配置 WinHTTP，使用独立代理或证书存储的应用需单独设置。
5. 完成测试后停止代理。应用会尝试恢复修改前保存的 HTTP 与 HTTPS 代理配置；如检测到代理设置已被其他程序或用户更改，会保留恢复记录并提示手动处理。

证书只应用于受控开发环境。不要导入生产设备，也不要分发给无关人员。

## 从 NetCapture 升级（macOS）

- 升级前请停止代理并完全退出旧版 NetCapture，避免两个版本同时修改系统代理。
- 首次使用 NetCat 启动时，如果新数据库尚不存在，应用会将旧目录 `~/Library/Application Support/com.netcapture.desktop/netcapture.sqlite3` 中的流量、Mock 规则和设置复制到新数据库。
- 旧目录、旧数据库和旧钥匙串记录不会被删除，便于回滚；已有 NetCat 数据不会被覆盖。
- 升级后会生成新的 `NetCat Local Root CA`。请重新导入并信任新证书，确认 HTTPS 抓包正常后，可从“钥匙串访问”手动删除旧的 `NetCapture Local Root CA`。
- 证书下载地址已切换为 `/__netcat/ca.crt`，HAR 扩展字段已切换为 `_netcat_*`，不再兼容旧名称。

## 手机接入

1. 电脑与手机连接同一可信 Wi-Fi；Windows 如遇防火墙提示，只为可信的专用网络允许代理入站。
2. 在“设置 > 代理配置”选择“手机抓包”并启动代理。手机模式不会启用系统代理，也不会记录电脑请求。
3. 将手机 Wi-Fi 的 HTTP 代理设为界面显示的电脑 IP 和端口。
4. 点击“生成配对二维码”，使用手机扫码。二维码 5 分钟内有效且只能使用一次；配对成功后设备会立即出现在列表和请求页设备 Tab 中。
5. 在配对成功页下载 CA，或访问 `http://<电脑-IP>:28086/__netcat/ca.crt`。iOS 安装描述文件后，还需进入“设置 > 通用 > 关于本机 > 证书信任设置”开启完全信任。
6. Android 7+ 应用默认可能不信任用户 CA；被测应用需在 Network Security Config 中明确允许用户证书。不同厂商系统的 Wi-Fi 代理入口可能位于当前网络的“修改网络 > 高级选项”。
7. 默认空闲 15 分钟后自动断开，可在设置中配置为 5–120 分钟。停止代理、手动断开或超时都会清除当前配对，下次使用需要重新扫码；已抓取的历史请求仍保留。

请求页按“全部 / 本机 / 各设备”切换流量。已断开但仍有历史请求的设备继续显示；可在设置中改名、重新配对、断开，或在确认请求数量后删除该设备的全部历史请求。

## JavaScript 脚本

“脚本”工作区分别管理代理脚本与 Frida 注入脚本。代理脚本按优先级从高到低执行，同优先级按创建时间执行；实时代理和请求重放使用同一顺序：请求脚本 → 弱网上行/故障 → Mock 或上游 → 响应脚本 → 弱网下行 → 持久化。Mock-aware 重放执行完整管线，直连重放仅绕过 Mock。

代理脚本可定义 `onRequest(ctx)` 和 `onResponse(ctx)`。返回值可包含 `method`、`url`、`headers`、`bodyText`、`bodyBase64`；`onRequest` 还可通过 `response: { status, headers, bodyText }` 直接生成响应。单次执行限制 100 ms 和 64 MB；脚本异常只回滚当前脚本并放行。解码后正文超过 10 MiB 或编码不支持时，`bodyEditable` 为 `false`，仍可修改 Header 和路由。

```javascript
function onRequest(ctx) {
  return { headers: { ...ctx.headers, 'x-netcat-script': 'enabled' } };
}

function onResponse(ctx) {
  return ctx.status === 200 ? { headers: { ...ctx.headers, 'cache-control': 'no-store' } } : {};
}
```

QuickJS 沙箱不提供 Node.js、npm、文件系统、网络或进程 API。脚本保存在本机，不会上传，也不会从网络获取脚本。

## 弱网模拟

网络请求顶部工具栏的限流配置可选择“无限流、高速 4G、低速 4G、3G、离线”或自定义配置。内置参数分别为：高速 4G 20 ms / 下行 9000 Kbps / 上行 1500 Kbps；低速 4G 150 ms / 1600 / 750 Kbps；3G 100 ms / 750 / 250 Kbps；离线为 100% 超时。

自定义配置支持延迟、抖动、上下行速率、超时/中断概率，以及方法和 URL 精确、包含或正则匹配。该功能只作用于 NetCat 代理层，不能模拟真实 IP 丢包、TCP 重排、拥塞窗口或链路层行为。

## SSL Pinning 与 Frida

NetCat 不安装 Frida、不部署 `frida-server`、不重打包或重签 APK/IPA，也不修改生产包。使用前自行安装兼容版本的 `frida-tools`，并在已授权测试设备上启动同主版本的 `frida-server`，或准备已集成 Gadget 的测试包：

```bash
python3 -m pip install frida-tools
frida --version
frida-ls-devices
```

在“SSL Pinning”工作区刷新环境，选择 USB 设备或填写 Gadget 地址，枚举应用/进程，选择 Spawn 或 Attach 与注入策略后启动。Android 内置策略覆盖 Conscrypt/TrustManager、OkHttp 和 WebView；iOS 覆盖 Security/NSURLSession、AFNetworking、Alamofire 和 TrustKit。框架升级、混淆、自定义 native TLS 或反调试可能需要自定义脚本。

## 故障恢复

### 手动关闭系统代理

macOS：若应用异常退出且自动恢复失败，先查看网络服务名称：

```bash
networksetup -listallnetworkservices
```

然后对实际使用的服务执行，例如 Wi-Fi：

```bash
networksetup -setwebproxystate "Wi-Fi" off
networksetup -setsecurewebproxystate "Wi-Fi" off
```

如果修改前使用了自定义代理，应在“系统设置 > 网络 > 当前网络 > 详细信息 > 代理”中恢复原服务器、端口和启用状态，而不是简单关闭。

Windows：以原来运行 NetCat 的同一用户打开“设置 > 网络和 Internet > 代理”，在“手动设置代理”中关闭“使用代理服务器”（仅适用于原先未使用代理的情况）。如原先使用自定义代理，应恢复原地址、端口、例外列表及启用状态；“自动设置代理”中的脚本地址和自动检测设置也应保留原值。修改后重新打开相关应用使其读取新设置。不要使用 `netsh winhttp reset proxy` 恢复此处的当前用户代理。

人工恢复完整原配置后，在 NetCat 设置中点击“重试恢复系统代理”。应用确认当前配置与保存的原配置完全一致后，会清理恢复记录，无需重写代理设置，之后可再次接管。若当前值仍不同于原值和 NetCat 写入值，应用会继续保留记录并提示冲突。

需要核对当前用户配置时，可在 PowerShell 只读查看：

```powershell
Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' |
  Select-Object ProxyEnable, ProxyServer, ProxyOverride, AutoConfigURL, AutoDetect
```

### 常见错误

- `端口不可用`：更换端口，或关闭占用该端口的程序。
- `CA 未生成`：打开证书面板并生成本地 CA。
- HTTPS 握手失败：确认 CA 已信任；目标应用也可能启用了证书固定。
- 上游连接失败或超时：确认目标服务器、DNS 和当前网络可访问。
- 系统代理恢复失败：按对应平台的手动恢复步骤处理；macOS 检查网络服务名称，Windows 检查是否以同一用户运行及设置是否被其他程序更改。
- Windows CA 不受信任：确认导入的是当前 NetCat CA，且位于当前用户的根证书存储；部分应用采用独立证书存储或证书固定。
- Windows 界面无法启动：确认 Microsoft Edge WebView2 Runtime 已安装，缺失时先安装 Evergreen Runtime 后重试。
- Frida 缺失：确认 `frida --version` 在启动 NetCat 的同一用户环境可执行，然后刷新环境。
- Frida 版本不一致：让客户端、`frida-server` 或 Gadget 使用兼容的同主版本；重启设备端服务后重新枚举。
- 设备断开或 Gadget 不可达：检查 USB 授权、端口转发、防火墙和 Gadget 地址；停止旧会话后重连。
- 脚本超时或语法错误：查看脚本测试结果与诊断日志；单次执行超过 100 ms 会回滚该脚本并放行。
- 请求异常变慢或超时：检查工具栏是否误启用了限流配置，切换“无限流”后复测。

## 卸载与删除 CA

卸载前必须先停止代理、关闭系统代理，确认网络已恢复后退出 NetCat。卸载器不会替代应用恢复代理或自动删除已信任的 CA。

### macOS

1. 完成上述停止和恢复操作。
2. 在“钥匙串访问”中搜索并删除 `NetCat Local Root CA`。
3. 删除“我的证书”或“证书”列表中同名的残留项。
4. 如需删除钥匙串中的 CA 私钥记录，可执行：

```bash
security delete-generic-password -s com.kaitehan.netcat -a root-ca-private-key
```

5. 删除应用数据目录 `~/Library/Application Support/com.kaitehan.netcat`，再将 `NetCat.app` 移到废纸篓。

### Windows

1. 停止代理并关闭系统代理，确认恢复成功；失败时先按“故障恢复”手动恢复。
2. 在证书面板移除当前 CA 的信任，再退出应用。也可运行 `certmgr.msc`，进入“当前用户 > 受信任的根证书颁发机构 > 证书”，核对指纹后只删除本应用生成的 `NetCat Local Root CA`，避免删除同名的其他证书。
3. 在“设置 > 应用 > 已安装的应用”（Windows 10 为“应用和功能”）中卸载 NetCat。
4. 如需彻底清除本地数据，删除 `%APPDATA%\com.kaitehan.netcat`；其中包含数据库、公开 CA 和 DPAPI 加密的私钥文件。先移除证书信任，再删除数据，以便核对证书身份。

删除应用数据会永久移除本地流量、Mock 规则和设置。HAR 或 CA 公钥导出文件不在该目录时需单独处理。

## 数据与安全边界

- 无账号、云同步、遥测和远程上传。
- 当前用户数据目录：macOS 为 `~/Library/Application Support/com.kaitehan.netcat`，Windows 为 `%APPDATA%\com.kaitehan.netcat`。数据库文件为 `netcat.sqlite3`，公开 CA 位于 `certificates` 子目录。
- CA 私钥不会进入 SQLite、日志、HAR、cURL 或证书导出文件。
- 暂停捕获只停止创建新展示记录，代理转发仍继续。
- 非法正则、Header、状态码、JSON 或延迟不会保存为运行时规则。
- TLS 失败只提示“证书未信任或应用可能启用证书固定”，不会声称已识别具体原因。
- Pinning、注入和脚本仅可用于自有或明确授权目标；本地脚本不会上传或联网获取。

## 下一步规划

- HTTP/2 完整解析、WebSocket 帧级调试，以及 QUIC / HTTP/3 支持。
- 自动更新、macOS 签名与公证，以及公开分发所需的合规工作。
- 基础断点改包仍受性能和稳定性门禁约束；若未通过 0.3.0 门禁，则作为后续版本候选，不视为 0.3.0 已交付能力。

## 参与贡献

欢迎提交缺陷报告、功能建议、文档改进和代码贡献。开始前请阅读 [贡献指南](CONTRIBUTING.md)；安全问题请按照 [安全策略](SECURITY.md) 私下报告，不要在公开 Issue 中披露漏洞细节。

建议为仓库配置以下 GitHub Topics：`http-proxy`、`https-proxy`、`network-debugging`、`api-testing`、`mock-server`、`request-replay`、`tauri`、`rust`、`vue`、`developer-tools`、`macos`、`windows`。

## 许可证

NetCat 使用 [Apache License 2.0](LICENSE) 开源。使用、修改和分发本项目时请遵守许可证条款。
