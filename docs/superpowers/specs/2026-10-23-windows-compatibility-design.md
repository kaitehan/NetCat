# NetCat Windows 兼容设计

## 背景与目标

NetCat 当前的系统代理、CA 私钥存储、系统诊断和端口占用诊断直接依赖 macOS。Windows 首版以 Windows 10 22H2 和 Windows 11 为支持范围，交付 x86_64 NSIS 安装包，并默认以当前用户权限运行。

本次改造采用“共享业务层 + 平台适配层”。目标是在不改变代理、Mock、重发和存储核心行为的前提下，为 Windows 提供原生、可恢复、可测试的平台能力，同时保持现有 macOS 行为稳定。

## 范围

首版包含：

- 当前用户级 Windows 系统代理接管、校验、冲突检测和恢复。
- 使用 Windows DPAPI 保护 CA 私钥。
- 将 NetCat CA 安装到当前用户根证书存储，并支持状态查询和安全移除。
- Windows 系统版本、端口占用进程和平台能力诊断。
- Windows NSIS 安装包和 WebView2 bootstrapper 策略。
- Windows 10 22H2、Windows 11 的自动化及手工兼容性验证。

首版不包含：

- 机器级代理或机器级证书配置。
- Windows on ARM。
- 企业组策略代理和 PAC 管理。
- Firefox 独立证书库。
- 静默安装根证书。

## 架构

平台能力通过统一的 `PlatformServices` 进入应用状态：

```text
Tauri commands / AppState
          |
          v
PlatformServices
  |- SystemProxy
  |- SecretStore
  |- CertificateTrust
  |- SystemDiagnostics
  `- PortInspector
          |
    +-----+-----+
    |           |
    v           v
  macOS       Windows
```

平台接口负责定义业务所需的最小能力，平台实现封装所有操作系统 API。Tauri 命令和代理业务不直接调用 `networksetup`、Keychain、WinINet 或 Windows 证书 API。

- `SystemProxy`：读取、接管、校验和恢复当前用户代理。
- `SecretStore`：保存和读取 CA 私钥。
- `CertificateTrust`：查询、安装和移除 NetCat CA 的信任状态。
- `SystemDiagnostics`：提供平台名称、版本和平台能力摘要。
- `PortInspector`：返回监听端口对应的 PID 和可访问的进程信息。

macOS 保留 `networksetup` 和 Keychain 实现。Windows 使用 `windows` crate 调用系统 API。其他平台提供明确的“不支持”实现，使编译和错误边界保持清晰。

## Windows 系统代理

Windows 后端读取当前用户 Internet Settings 中与手动代理有关的完整配置，包括 `ProxyEnable`、`ProxyServer`、`ProxyOverride` 和自动配置状态。快照写入 SQLite 设置表，记录平台、格式版本、接管时间、原始配置和 NetCat 代理端口。

启用流程：

1. 检查是否已有有效接管快照。
2. 读取并持久化原始配置。
3. 将 HTTP 和 HTTPS 代理设置为 `127.0.0.1:<port>`，保留必要的 bypass 配置。
4. 发送 WinINet 设置变更通知。
5. 重新读取配置并校验。
6. 任一步骤失败时使用快照回滚，并保留可诊断的错误信息。

恢复流程遵循“不覆盖用户后续修改”的原则：

- 当前配置仍与 NetCat 写入值一致时，恢复完整快照并删除记录。
- 用户或其他程序在运行期间修改配置时，不自动覆盖新值，并返回代理冲突状态。
- 异常退出后，下次启动检查遗留快照；仅在当前值仍属于 NetCat 时自动恢复。
- macOS 现有快照继续兼容，新增快照通过平台和版本字段区分。

系统代理修改失败不影响本地代理监听和手动代理接入。

## CA 私钥与证书信任

`SecretStore` 保留现有统一接口。macOS 继续使用 Keychain；Windows 使用当前用户上下文的 DPAPI 加密私钥，并将密文保存在应用数据目录。私钥仅在代理启动和签发站点证书时解密，不进入日志或诊断包。

Windows `CertificateTrust` 将 CA 公钥安装到 `CurrentUser\\Root`：

- 安装操作必须由用户明确触发。
- 安装和状态查询以 SHA-256 指纹为准，避免只按证书名称判断。
- 移除操作只删除指纹匹配且由 NetCat 管理的证书。
- 同名但指纹不同的证书不会被删除。

证书状态分为：未生成、已生成未信任、已信任、证书与私钥不一致、访问失败。诊断信息只包含指纹和信任状态，不包含证书正文、DPAPI 密文或私钥。

macOS 首期保留现有证书导出和人工信任流程，不在本次改造中改变用户行为。

## 系统诊断与端口检查

诊断信息改为通过 `SystemDiagnostics` 获取统一结构，避免在 Tauri 命令内直接调用 `sw_vers`。

- macOS 继续使用现有系统工具实现。
- Windows 使用系统 API 获取操作系统版本。
- 诊断包记录平台、版本、架构、代理接管状态和 CA 信任状态。

Windows 端口诊断通过 TCP 表查询监听 PID，并在权限允许时读取进程名和路径。无法访问其他用户或受保护进程时，仍返回 PID 和明确的权限提示，不尝试终止进程。

## 前端体验

设置页根据平台能力展示操作：

- Windows 证书区提供“安装信任”和“移除信任”，展示证书指纹及信任状态。
- 系统代理冲突时展示当前状态，并提供基于已保存快照的显式恢复入口。
- 不支持的平台隐藏不可执行操作，并显示原因。
- 接入说明按 macOS、Windows、iOS 和 Android 区分，避免跨平台文案混用。

共享类型增加平台信息、证书信任状态和完整的系统代理状态。既有命令名称尽量保持兼容，新增能力使用独立命令，降低前端迁移风险。

Vue 改动继续使用 Composition API 和异步组件；组件样式遵循多行 CSS 格式。

## 错误处理与安全边界

- 平台操作使用稳定错误码，并携带面向用户的处理建议。
- 修改后必须读取校验，失败时优先回滚。
- 恢复前必须确认当前配置仍由 NetCat 管理。
- 系统代理、证书和 DPAPI 操作写入脱敏诊断日志。
- 不记录代理凭据、请求/响应正文、认证头、Cookie、Token、证书私钥或 DPAPI 密文。
- Windows 操作不依赖 PowerShell，避免执行策略、语言环境和命令转义差异。

## 构建与分发

Windows 平台依赖放入 Cargo target-specific dependencies，macOS 构建不链接 Windows API。使用 `windows` crate 按需启用 WinINet、Cryptography、Certificate、Networking 和 Process 相关能力。

Tauri 增加 Windows NSIS 配置、现有 `.ico` 资源引用和 WebView2 bootstrapper 安装策略。首版目标为 `x86_64-pc-windows-msvc`。

README 更新支持平台和 Windows 使用说明，包括当前用户级代理、CA 信任、异常恢复以及卸载前停止代理的提示。

## 测试与验收

平台后端使用可替换的系统 API 适配器测试，单元测试不得直接修改开发机代理或证书库。

自动化覆盖：

- 代理快照、写入后校验、失败回滚和异常启动恢复。
- 用户或其他程序修改代理后的冲突处理。
- DPAPI 加解密往返和损坏密文错误。
- CA 指纹查重、信任状态、安装及精确移除。
- Windows 端口监听 PID 和受限进程降级行为。
- 前端平台能力渲染和证书操作状态。

Windows CI 执行前端测试、Rust 测试、Windows target 检查和 Tauri NSIS 构建。macOS 执行完整回归，确保现有代理恢复、Keychain 和 DMG 构建不受影响。

手工验收在 Windows 10 22H2 和 Windows 11 各执行一次，覆盖安装、启动代理、HTTP/HTTPS 抓包、Mock、重发、CA 信任、异常退出恢复、诊断包导出和卸载。诊断包继续执行敏感信息扫描。

## 实施顺序

1. 建立平台接口和 macOS 行为回归测试。
2. 实现 Windows DPAPI 私钥存储和 CA 信任。
3. 实现 Windows 系统代理快照、接管、校验和恢复。
4. 实现 Windows 诊断与端口检查。
5. 接入前端平台状态和证书操作。
6. 配置 NSIS、Windows CI、README 和跨平台验收。
