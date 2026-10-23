# NetCat Windows Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 NetCat 增加 Windows 10 22H2 与 Windows 11 的当前用户级系统代理、CA 私钥保护、CA 信任、诊断能力和 x86_64 NSIS 安装包，同时保持 macOS 行为不变。

**Architecture:** 将操作系统能力收敛到 `PlatformServices`，通过小型 trait 连接共享业务层与 macOS/Windows 原生实现。系统代理由共享协调器负责快照、校验和恢复，平台后端只负责读取和写入系统状态；CA 管理继续负责生成证书，平台服务负责私钥保护和证书信任。

**Tech Stack:** Rust 1.85、Tauri 2.11、`windows` crate、`winreg`、Vue 3 Composition API、TypeScript、Vitest、GitHub Actions、NSIS

**Spec:** `docs/superpowers/specs/2026-10-23-windows-compatibility-design.md`

## Global Constraints

- 支持范围固定为 Windows 10 22H2 和 Windows 11，首版目标为 `x86_64-pc-windows-msvc`。
- Windows 平台能力默认使用当前用户权限，不写机器级代理和机器级证书存储。
- 根证书安装必须由用户明确触发，不提供静默信任流程。
- Windows 平台操作不依赖 PowerShell。
- 系统代理恢复不得覆盖用户或其他程序在 NetCat 运行期间做出的新修改。
- 私钥、DPAPI 密文、请求/响应正文、Cookie、Authorization 和 Token 不得进入诊断包或日志。
- macOS 现有 `networksetup`、Keychain、DMG 和证书导出行为必须保持可用。
- Vue 文件使用 Composition API 与 `<script setup lang="ts">`，所有 CSS 属性保持多行格式。
- 每个提交信息使用中文 Conventional Commits 格式。

## File Structure

- `src-tauri/src/platform/mod.rs`：平台能力接口、共享状态类型和当前平台服务构造入口。
- `src-tauri/src/platform/macos.rs`：macOS 平台服务装配、系统信息和端口诊断。
- `src-tauri/src/platform/windows/mod.rs`：Windows 平台服务装配。
- `src-tauri/src/platform/windows/secrets.rs`：DPAPI 私钥保护与文件存储。
- `src-tauri/src/platform/windows/certificates.rs`：当前用户根证书存储的查询、安装和精确移除。
- `src-tauri/src/platform/windows/system_proxy.rs`：Internet Settings 读取写入和 WinINet 变更通知。
- `src-tauri/src/platform/windows/diagnostics.rs`：Windows 版本、架构和端口占用进程查询。
- `src-tauri/src/system_proxy.rs`：平台无关的快照、接管、验证、冲突和恢复协调器。
- `src-tauri/src/ca.rs`：平台无关 CA 生成与 `Arc<dyn SecretStore>` 接入。
- `src-tauri/src/models.rs`：平台信息、证书信任和系统代理状态的序列化模型。
- `src-tauri/src/app.rs`：平台服务注入、Tauri 命令及诊断包元数据。
- `src-tauri/src/proxy.rs`：通过平台端口检查接口生成端口占用建议。
- `src/shared/types.ts`：Rust 模型对应的 TypeScript 类型。
- `src/api/desktop.ts`：平台信息、证书信任和系统代理状态命令封装。
- `src/composables/useNetCat.ts`：平台状态加载和证书信任操作。
- `src/features/certificate/CertificatePanel.vue`：Windows CA 信任状态与操作。
- `src/features/settings/SettingsPage.vue`：平台代理状态、冲突提示和平台化说明。
- `src-tauri/tauri.windows.conf.json`：NSIS 和 WebView2 Windows 打包配置。
- `.github/workflows/windows.yml`：Windows 构建和测试门禁。
- `README.md`：Windows 开发、使用、恢复和卸载说明。

---

### Task 1: 建立平台接口并保护 macOS 现有行为

**Files:**
- Create: `src-tauri/src/platform/mod.rs`
- Create: `src-tauri/src/platform/macos.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/ca.rs`
- Modify: `src-tauri/src/system_proxy.rs`
- Test: `src-tauri/src/system_proxy.rs`
- Test: `src-tauri/src/ca.rs`

**Interfaces:**
- Produces: `PlatformServices::current(store: Arc<Store>, data_dir: &Path) -> Result<Self, AppError>`
- Produces: `SecretStore::{save_private_key, load_private_key}` as an object-safe `Send + Sync` trait.
- Produces: `SystemProxyBackend::{platform, capture, apply, is_managed, restore}`.
- Produces: `SystemProxyManager::new(store: Arc<Store>, backend: Arc<dyn SystemProxyBackend>)`.

- [ ] **Step 1: 为共享代理协调器写失败测试**

在 `src-tauri/src/system_proxy.rs` 的测试模块加入 `FakeProxyBackend`，覆盖启用保存版本化快照、写入失败回滚、校验失败回滚、恢复时发现用户修改则返回冲突且保留快照。

```rust
#[test]
fn restore_preserves_snapshot_when_current_proxy_was_changed() {
    let fixture = ProxyFixture::managed();
    fixture.backend.set_external_change();

    let error = fixture.manager.restore().unwrap_err();

    assert_eq!(error.code, "SYSTEM_PROXY_CONFLICT");
    assert!(fixture.manager.is_managed().unwrap());
    assert_eq!(fixture.backend.restore_calls(), 0);
}
```

- [ ] **Step 2: 运行测试并确认旧实现无法满足接口**

Run: `cargo test --manifest-path src-tauri/Cargo.toml system_proxy::tests -- --nocapture`

Expected: FAIL，原因是 `SystemProxyBackend`、`ProxyFixture` 和版本化快照尚不存在。

- [ ] **Step 3: 实现共享接口并迁移 macOS 后端**

在 `platform/mod.rs` 定义对象安全接口和平台服务：

```rust
pub trait SecretStore: Send + Sync {
    fn save_private_key(&self, value: &str) -> Result<(), AppError>;
    fn load_private_key(&self) -> Result<Option<String>, AppError>;
}

pub trait SystemProxyBackend: Send + Sync {
    fn platform(&self) -> &'static str;
    fn capture(&self) -> Result<serde_json::Value, AppError>;
    fn apply(&self, port: u16) -> Result<(), AppError>;
    fn is_managed(&self, port: u16) -> Result<bool, AppError>;
    fn restore(&self, snapshot: &serde_json::Value) -> Result<(), AppError>;
}
```

将 `CaManager<S>` 改为持有 `Arc<dyn SecretStore>` 的非泛型 `CaManager`。把现有 `networksetup`、`sw_vers` 和 Keychain 构造移动到 macOS 平台实现，保持现有错误码和用户提示。

- [ ] **Step 4: 运行共享与 macOS 回归测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml system_proxy::tests ca::tests -- --nocapture`

Expected: PASS，快照冲突测试不调用恢复，CA 内存存储测试继续通过。

- [ ] **Step 5: 提交平台骨架**

```bash
git add src-tauri/src/platform src-tauri/src/lib.rs src-tauri/src/ca.rs src-tauri/src/system_proxy.rs
git commit -m "refactor(platform): 抽离系统平台能力接口"
```

### Task 2: 实现 Windows DPAPI 私钥存储

**Files:**
- Create: `src-tauri/src/platform/windows/mod.rs`
- Create: `src-tauri/src/platform/windows/secrets.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/platform/windows/secrets.rs`

**Interfaces:**
- Consumes: object-safe `SecretStore` from Task 1.
- Produces: `DpapiSecretStore::new(path: PathBuf) -> Self`.
- Produces: Windows-only `protect_for_current_user` and `unprotect_for_current_user` helpers.

- [ ] **Step 1: 写 Windows 私钥存储测试**

```rust
#[cfg(target_os = "windows")]
#[test]
fn saves_encrypted_key_and_loads_plaintext() {
    let directory = tempfile::tempdir().unwrap();
    let store = DpapiSecretStore::new(directory.path().join("ca-key.dpapi"));

    store.save_private_key("private-key-pem").unwrap();

    assert_eq!(store.load_private_key().unwrap().as_deref(), Some("private-key-pem"));
    assert_ne!(std::fs::read(store.path()).unwrap(), b"private-key-pem");
}
```

同时添加损坏密文返回 `DPAPI_DECRYPT_ERROR`、不存在文件返回 `Ok(None)` 的测试。

- [ ] **Step 2: 在 Windows runner 上确认测试失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc platform::windows::secrets -- --nocapture`

Expected: FAIL，原因是 `DpapiSecretStore` 和 Windows 依赖尚不存在。

- [ ] **Step 3: 添加目标平台依赖并实现 DPAPI**

在 `Cargo.toml` 使用目标依赖，避免 macOS 链接 Windows API：

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.62", features = ["Win32_Foundation", "Win32_Security_Cryptography"] }
winreg = "0.55"
```

实现 `CryptProtectData`、`CryptUnprotectData` 和 `LocalFree` 的安全包装。写入使用同目录临时文件后原子替换，文件内容带固定格式版本字节，空明文和 UTF-8 解码失败均返回稳定错误码。

- [ ] **Step 4: 运行 Windows 测试和 macOS 编译检查**

Run on Windows: `cargo test --manifest-path src-tauri/Cargo.toml platform::windows::secrets -- --nocapture`

Run on macOS: `cargo check --manifest-path src-tauri/Cargo.toml`

Expected: Windows DPAPI 测试 PASS；macOS 不构建 Windows 模块且检查 PASS。

- [ ] **Step 5: 提交 DPAPI 存储**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/platform/windows
git commit -m "feat(windows): 使用 DPAPI 保护 CA 私钥"
```

### Task 3: 实现 Windows 当前用户 CA 信任

**Files:**
- Create: `src-tauri/src/platform/windows/certificates.rs`
- Modify: `src-tauri/src/platform/mod.rs`
- Modify: `src-tauri/src/platform/windows/mod.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/platform/windows/certificates.rs`

**Interfaces:**
- Produces: `CertificateTrust::{status, install, remove}`.
- Produces: `CertificateTrustState::{Unsupported, NotGenerated, NotTrusted, Trusted, Mismatch, Unavailable}`.
- Consumes: PEM certificate path and SHA-256 fingerprint from `CaManager`.

- [ ] **Step 1: 为指纹匹配和安全移除写失败测试**

使用内存 `CertificateStoreApi` 测试同名不同指纹证书不会被视为已信任，也不会被删除。

```rust
#[test]
fn remove_deletes_only_the_matching_fingerprint() {
    let api = MemoryCertificateStore::with_certificates([
        certificate("NetCat Local Root CA", "AA"),
        certificate("NetCat Local Root CA", "BB"),
    ]);
    let trust = WindowsCertificateTrust::new(api.clone());

    trust.remove(&certificate_der("AA"), "AA").unwrap();

    assert_eq!(api.fingerprints(), vec!["BB"]);
}
```

- [ ] **Step 2: 运行证书测试并确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml platform::windows::certificates -- --nocapture`

Expected: FAIL，原因是证书接口和 Windows 实现尚不存在。

- [ ] **Step 3: 实现 CurrentUser Root 操作**

使用 `CertOpenStore` 打开当前用户 `ROOT`，使用 `CertAddEncodedCertificateToStore` 安装 DER 证书，枚举证书并用 SHA-256 指纹匹配，移除时仅调用匹配上下文的 `CertDeleteCertificateFromStore`。PEM 到 DER 使用 `rustls-pemfile`，只接受一个 X.509 证书块。

```rust
pub trait CertificateTrust: Send + Sync {
    fn status(&self, certificate_pem: &[u8], fingerprint: &str)
        -> Result<CertificateTrustState, AppError>;
    fn install(&self, certificate_pem: &[u8], fingerprint: &str)
        -> Result<CertificateTrustState, AppError>;
    fn remove(&self, certificate_pem: &[u8], fingerprint: &str)
        -> Result<CertificateTrustState, AppError>;
}
```

- [ ] **Step 4: 运行证书单元测试和 Windows 编译检查**

Run: `cargo test --manifest-path src-tauri/Cargo.toml platform::windows::certificates -- --nocapture`

Run: `cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc`

Expected: 指纹测试 PASS，Windows target 检查 PASS。

- [ ] **Step 5: 提交证书信任实现**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/platform
git commit -m "feat(windows): 支持当前用户 CA 信任管理"
```

### Task 4: 实现 Windows 系统代理接管与恢复

**Files:**
- Create: `src-tauri/src/platform/windows/system_proxy.rs`
- Modify: `src-tauri/src/platform/windows/mod.rs`
- Modify: `src-tauri/src/system_proxy.rs`
- Test: `src-tauri/src/platform/windows/system_proxy.rs`
- Test: `src-tauri/src/system_proxy.rs`

**Interfaces:**
- Consumes: `SystemProxyBackend` and versioned snapshot coordinator from Task 1.
- Produces: `WindowsSystemProxyBackend::new(registry: Arc<dyn InternetSettingsApi>)`.
- Produces: serializable `WindowsProxyConfiguration` with proxy, bypass and auto-config fields.

- [ ] **Step 1: 写注册表映射和变更通知失败测试**

```rust
#[test]
fn apply_sets_http_and_https_without_discarding_bypass() {
    let registry = MemoryInternetSettings::from(configuration_with_bypass("<local>"));
    let backend = WindowsSystemProxyBackend::new(registry.clone());

    backend.apply(28086).unwrap();

    assert_eq!(registry.proxy_server(), "http=127.0.0.1:28086;https=127.0.0.1:28086");
    assert_eq!(registry.proxy_override(), "<local>");
    assert_eq!(registry.notifications(), 1);
}
```

补充 `is_managed` 拒绝不同端口、缺失注册表值可往返恢复、通知失败触发共享协调器回滚的测试。

- [ ] **Step 2: 运行 Windows 代理测试并确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml platform::windows::system_proxy system_proxy::tests -- --nocapture`

Expected: FAIL，原因是 Windows 后端尚不存在。

- [ ] **Step 3: 实现 Internet Settings 与 WinINet 通知**

使用 `winreg` 读写 `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings`。读取值时保留“不存在”和“存在但为空”的区别；应用时设置 `ProxyEnable=1`、`ProxyServer`，不修改 PAC URL 和自动检测字段；恢复时逐项恢复或删除原本不存在的值。

写入后调用：

```rust
InternetSetOptionW(None, INTERNET_OPTION_SETTINGS_CHANGED, None, 0)?;
InternetSetOptionW(None, INTERNET_OPTION_REFRESH, None, 0)?;
```

共享协调器在 `apply` 后调用 `is_managed(port)`；失败时恢复原快照。恢复时如果当前配置不再属于 NetCat，返回 `SYSTEM_PROXY_CONFLICT` 并保留快照。

- [ ] **Step 4: 运行代理测试与跨平台检查**

Run: `cargo test --manifest-path src-tauri/Cargo.toml system_proxy::tests -- --nocapture`

Run on Windows: `cargo test --manifest-path src-tauri/Cargo.toml platform::windows::system_proxy -- --nocapture`

Expected: 共享测试和 Windows 注册表适配器测试全部 PASS。

- [ ] **Step 5: 提交 Windows 系统代理**

```bash
git add src-tauri/src/platform/windows src-tauri/src/system_proxy.rs
git commit -m "feat(windows): 支持系统代理接管与安全恢复"
```

### Task 5: 接入平台诊断、端口查询和 Tauri 命令

**Files:**
- Create: `src-tauri/src/platform/windows/diagnostics.rs`
- Modify: `src-tauri/src/platform/mod.rs`
- Modify: `src-tauri/src/platform/macos.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/app.rs`
- Modify: `src-tauri/src/proxy.rs`
- Test: `src-tauri/src/app.rs`
- Test: `src-tauri/src/proxy.rs`

**Interfaces:**
- Produces: `PlatformInfo { os, version, arch, certificate_trust_supported }`.
- Produces: `SystemProxyStatus { managed, recovery_pending, conflict }`.
- Produces Tauri commands: `get_platform_info`, `get_system_proxy_status`, `get_certificate_trust_status`, `install_ca_trust`, `remove_ca_trust`.
- Consumes: `PlatformServices`, `CaManager`, `SystemProxyManager` and `PortInspector`.

- [ ] **Step 1: 写平台元数据与命令失败测试**

```rust
#[test]
fn diagnostic_metadata_uses_platform_info_without_private_certificate_data() {
    let metadata = diagnostic_metadata(&fixture_state());

    assert_eq!(metadata["platform"]["os"], "windows");
    assert!(metadata["ca"].get("fingerprint").is_some());
    assert!(metadata["ca"].get("privateKey").is_none());
}
```

在 `proxy.rs` 为 `PortInspector` fake 添加测试，验证端口错误包含 `process.exe (PID 42)`，无权限读取名称时退化为 `PID 42`。

- [ ] **Step 2: 运行应用与代理测试并确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml app::tests proxy::tests -- --nocapture`

Expected: FAIL，原因是平台模型、命令和端口接口尚未接入。

- [ ] **Step 3: 实现平台命令和 Windows 诊断**

Windows 版本通过版本 API 获取，端口占用通过 `GetExtendedTcpTable` 找到监听 PID，并使用 `OpenProcess` 与 `QueryFullProcessImageNameW` 获取可访问的进程名。将 `ProxyService` 构造函数增加 `Arc<dyn PortInspector>` 参数。

`AppState` 改为持有 `PlatformServices` 和非泛型 `CaManager`；启动时调用安全恢复，退出时尽力停止代理并恢复。诊断包统一写入 `platform` 字段，不再写死 `macOS`。

- [ ] **Step 4: 运行 Rust 全量测试与双平台检查**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Run: `cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc`

Expected: Rust 测试全部 PASS，Windows target 检查 PASS，诊断 JSON 不含私钥和证书正文。

- [ ] **Step 5: 提交平台命令和诊断**

```bash
git add src-tauri/src/app.rs src-tauri/src/models.rs src-tauri/src/proxy.rs src-tauri/src/platform
git commit -m "feat(windows): 接入平台诊断与端口检查"
```

### Task 6: 增加 Windows 设置与证书交互

**Files:**
- Modify: `src/shared/types.ts`
- Modify: `src/api/desktop.ts`
- Modify: `src/composables/useNetCat.ts`
- Modify: `src/App.vue`
- Modify: `src/features/settings/SettingsPage.vue`
- Modify: `src/features/certificate/CertificatePanel.vue`
- Modify: `src/api/desktop.test.ts`
- Modify: `src/App.test.ts`
- Modify: `src/features/settings/SettingsPage.test.ts`
- Modify: `src/features/certificate/CertificatePanel.test.ts`

**Interfaces:**
- Consumes: Tauri commands and serialized models from Task 5.
- Produces: `PlatformInfo`, `SystemProxyStatus`, `CertificateTrustState` TypeScript types.
- Produces: `trustCa()` and `removeCaTrust()` actions in `useNetCat`.

- [ ] **Step 1: 读取 Vue 编码与测试技能并写失败测试**

开始修改 Vue 文件前完整读取 `vue-best-practices` 和 `vue-testing-best-practices`。增加组件测试：Windows 已生成但未信任时显示“安装信任”，点击触发 `trust`；macOS 显示现有钥匙串说明；代理冲突时显示恢复提示且不会把开关错误标记为已启用。

```ts
it('offers current-user trust installation on Windows', async () => {
  render(CertificatePanel, {
    props: {
      platform: { os: 'windows', version: '10.0.19045', arch: 'x86_64', certificateTrustSupported: true },
      trustState: 'notTrusted',
      caStatus: generatedCa,
      proxyStatus: stoppedStatus,
      allowLan: false,
      busy: false
    }
  })

  await fireEvent.click(screen.getByRole('button', { name: '安装信任' }))
  expect(emitted().trust).toHaveLength(1)
})
```

- [ ] **Step 2: 运行前端定向测试并确认失败**

Run: `npm test -- src/api/desktop.test.ts src/features/certificate/CertificatePanel.test.ts src/features/settings/SettingsPage.test.ts src/App.test.ts`

Expected: FAIL，原因是平台 props、信任事件和桌面 API 尚不存在。

- [ ] **Step 3: 接入类型、API、状态和平台化界面**

在 `DesktopApi` 增加：

```ts
getPlatformInfo(): Promise<PlatformInfo>
getSystemProxyStatus(): Promise<SystemProxyStatus>
getCertificateTrustStatus(): Promise<CertificateTrustState>
installCaTrust(): Promise<CertificateTrustState>
removeCaTrust(): Promise<CertificateTrustState>
```

初始化时并行加载这些状态。Windows 证书面板使用明确按钮触发安装和移除；macOS 保留导出与钥匙串步骤。设置页展示代理冲突和恢复入口。所有事件通过 props 向下、事件向上传递，不在组件内直接调用桌面 API。

- [ ] **Step 4: 运行前端测试和构建**

Run: `npm test`

Run: `npm run build:budget`

Expected: Vitest 全部 PASS，TypeScript 检查 PASS，现有 bundle 预算继续通过。

- [ ] **Step 5: 提交 Windows 界面**

```bash
git add src/shared/types.ts src/api src/composables src/App.vue src/features/settings src/features/certificate
git commit -m "feat(ui): 增加 Windows 代理与证书设置"
```

### Task 7: 配置 NSIS、Windows CI 和使用文档

**Files:**
- Create: `src-tauri/tauri.windows.conf.json`
- Create: `.github/workflows/windows.yml`
- Modify: `README.md`
- Modify: `package.json`
- Test: `src-tauri/tauri.windows.conf.json`
- Test: `.github/workflows/windows.yml`

**Interfaces:**
- Consumes: Windows target 可编译应用和现有 `icon.ico`。
- Produces: `npm run tauri:build:windows` 和 Windows CI artifact。

- [ ] **Step 1: 添加可验证的 Windows 构建脚本与配置检查**

在 `package.json` 增加：

```json
"tauri:build:windows": "tauri build --target x86_64-pc-windows-msvc --bundles nsis --config src-tauri/tauri.windows.conf.json"
```

`tauri.windows.conf.json` 指定 `targets: ["nsis"]`、`webviewInstallMode.type: "downloadBootstrapper"` 和现有 `icons/icon.ico`。

- [ ] **Step 2: 先运行配置检查并记录缺失项**

Run: `npm run tauri -- info`

Run: `test -f .github/workflows/windows.yml && test -f src-tauri/tauri.windows.conf.json`

Expected: 第二条命令在文件创建前 FAIL，用于确认构建门禁尚未存在。

- [ ] **Step 3: 添加 Windows CI 与 README**

工作流使用 `windows-2022`，依次运行 `npm ci`、`npm test`、`npm run build:budget`、`cargo test --manifest-path src-tauri/Cargo.toml` 和 `npm run tauri:build:windows`，上传生成的 `.exe` 安装包。

README 将支持平台改为 macOS 12+、Windows 10 22H2 和 Windows 11；分别列出 Windows CA 信任、系统代理手工恢复、当前用户数据目录、卸载步骤和 WebView2 前提。下一步规划移除已启动的 Windows 兼容项。

- [ ] **Step 4: 验证配置、文档和 Windows 构建**

Run on Windows: `npm run tauri:build:windows`

Run on macOS: `npm run tauri -- build --bundles app,dmg`

Expected: Windows 生成 NSIS `.exe`，macOS 继续生成 `.app` 和 DMG。

- [ ] **Step 5: 提交打包和文档**

```bash
git add .github/workflows/windows.yml src-tauri/tauri.windows.conf.json package.json README.md
git commit -m "build(windows): 添加 NSIS 打包与兼容性检查"
```

### Task 8: 完成跨平台回归与发布验收

**Files:**
- Create: `docs/testing/windows-compatibility-checklist.md`
- Modify: `docs/superpowers/plans/2026-10-23-windows-compatibility.md`

**Interfaces:**
- Consumes: Tasks 1-7 的 Windows 和 macOS 构建产物。
- Produces: 可复现的 Windows 10 22H2、Windows 11 与 macOS 验收记录。

- [x] **Step 1: 创建手工验收清单**

清单逐项记录操作系统版本、安装包 SHA-256、HTTP/HTTPS 抓包、Mock、重发、CA 安装/移除、端口占用提示、异常退出恢复、用户修改代理冲突、诊断包敏感信息扫描和卸载结果。每项使用“通过/失败/未执行”三态并附证据路径。

- [ ] **Step 2: 运行完整自动化门禁**

Run:

```bash
npm test
npm run build:budget
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc
```

Expected: 所有命令退出码为 0，无失败测试，bundle 预算通过。

- [ ] **Step 3: 执行平台安装包验收**

Windows 10 22H2 和 Windows 11 分别使用 NSIS 安装包完成清单；macOS 使用 `.app` 和 DMG 完成代理恢复、Keychain 和现有功能回归。任何未执行项必须保留为“未执行”，不得记为通过。

- [ ] **Step 4: 检查诊断包安全边界**

解压三个平台验收产生的诊断包并搜索敏感字段：

```bash
rg -n -i "authorization|cookie|token|private key|BEGIN (RSA |EC )?PRIVATE KEY|dpapi" <diagnostic-directory>
```

Expected: 无敏感正文或私钥命中；允许的错误字段名必须人工确认其值不含凭据。

- [x] **Step 5: 提交验收记录**

```bash
git add docs/testing/windows-compatibility-checklist.md docs/superpowers/plans/2026-10-23-windows-compatibility.md
git commit -m "test(windows): 记录跨平台兼容性验收结果"
```

### Task 8 执行记录（2026-10-23）

被测基线为 `a64f8e8eafe2918ded7877f69dd49e52cf373f2f`（NetCat 0.3.0）。宿主为 macOS 26.2（25C56）arm64，Node 24.19.0（arm64），Rust/Cargo 1.85.0（host 为 `x86_64-apple-darwin`）。本次默认 Rust 测试/构建使用 x86_64 target，未做原生 arm64 Rust 验收。详细三态清单与后续实机操作步骤见 [跨平台验收记录](../../testing/windows-compatibility-checklist.md)。

- [x] 创建验收清单，记录 Windows 10 22H2、Windows 11 与 macOS 的环境、安装包身份、功能、代理恢复、证书、诊断扫描和卸载项目；未运行项目保持“未执行”。
- [x] `npm test`：exit 0，22 个文件 / 92 passed / 0 failed。
- [x] `npm run build:budget`：exit 0，类型检查、生产构建与体积预算通过，Monaco assets 4.51 MB；保留已有大 chunk 提示。
- [x] `cargo test --manifest-path src-tauri/Cargo.toml`：exit 0，92 passed / 0 failed / 2 ignored；未执行性能基准和 60 秒压力测试。
- [ ] 完整 Windows target 门禁通过：本次实际执行 `cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc`，exit 101；`ring` 缺 `assert.h`，`aws-lc-sys` 缺 `windows.h`，属于本机缺少 Windows SDK/MSVC C 工具链。Tasks 2–5 的独立 harness 已对实际 Windows 源文件做过 target 类型检查，不能代替整应用检查或原生 API 执行。
- [ ] Windows 10 22H2 / Windows 11：无实机与 NSIS 产物，NSIS 构建、DPAPI/证书/注册表/端口原生 API、安装运行和卸载均未执行，未推断通过。
- [x] macOS `npm run tauri -- build --bundles app,dmg`：exit 0，产出 `NetCat.app`（27.03 MiB）与 `NetCat_0.3.0_x64.dmg`（14.56 MiB）；`file` 确认 x86_64，Info.plist 检查和 `hdiutil verify` 通过。DMG SHA-256 为 `1e37a385e1e6f10c8945f8efcd63b7c4efd1b155edea43eccbafa49c35c0e067`，完整路径与可执行文件校验值见验收清单。
- [ ] macOS app/DMG 安装后代理恢复、Keychain、现有功能及卸载的手工回归均未执行；本次未启动 app 或改动系统设置。
- [ ] 三个平台真实验收诊断包扫描：没有实际验收包，均未执行；不以测试 fixture 或人工样本代替。

本次原始命令日志保留在被 Git 忽略的 `.superpowers/sdd/2026-10-23-windows-compatibility/task-8-evidence/`。自动化门禁整体、平台安装包手工验收与诊断包扫描尚未完成，因此 Task 8 的 Step 2–4 保持未勾选，不能据此宣称三平台发布验收通过。
