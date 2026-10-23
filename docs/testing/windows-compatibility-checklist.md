# Windows 兼容性与跨平台发布验收

## 本次执行范围

- 日期：2026-10-23（Asia/Shanghai）。被测产品：NetCat 0.3.0；代码基线：`a64f8e8eafe2918ded7877f69dd49e52cf373f2f`。
- 可用主机：macOS 26.2（25C56），arm64；Node 24.19.0，npm 10.9.2，Rust/Cargo 1.85.0，Apple Command Line Tools。
- Rust 工具链 host 为 `x86_64-apple-darwin`，本次未指定 target 的 Rust 测试和 macOS release 构建因此使用 x86_64；Node 的 `process.arch` 为 arm64。物理宿主架构不代表产物架构，本次没有执行原生 arm64 Rust 构建/测试。
- Windows 10 22H2 和 Windows 11 主机均不可用，没有运行远端 Windows CI，没有 NSIS 安装包。
- 状态仅使用“通过 / 失败 / 未执行”。自动化测试、目标类型检查、安装包构建、手工安装运行验收分别记录，不能互相替代。
- 本次结论：尚未完成跨平台发布验收。Windows 完整目标检查受本机工具链限制失败，两个 Windows 平台和 macOS 的安装运行、系统代理、信任与卸载手工验收均未执行。

## 证据索引

以下路径均相对仓库根目录。`E` 表示本次本地证据目录 `.superpowers/sdd/2026-10-23-windows-compatibility/task-8-evidence/`，它被 Git 忽略，日志不随本记录提交；以下摘要为已提交的验收结果。交接原始证据时需另行保留此目录。

| 编号 | 本地证据路径 | 内容 |
| --- | --- | --- |
| E0 | `E/environment.log` | 日期、代码 SHA、macOS 版本、架构和工具链版本 |
| E1 | `E/npm-test.log` | 前端全量测试 |
| E2 | `E/build-budget.log` | 类型检查、前端生产构建和体积预算 |
| E3 | `E/cargo-test.log` | macOS 宿主 Rust 全量测试与忽略项 |
| E4 | `E/windows-cargo-check.log` | 完整 Windows target 检查的 C 依赖工具链失败 |
| E5 | `E/macos-bundle.log` | macOS app/DMG 构建 |
| E0a | `E/toolchain-architecture.log` | Rust host 与 Node 进程架构 |
| E6 | `E/macos-artifacts.log` | app 结构、x86_64 架构、SHA-256 和 DMG 校验 |
| E7 | `E/document-review.log` | 文档结果与测试日志及未执行状态的一致性核对 |

手工验收表中的 `M/<平台>/<项目>/` 是未来应采集的相对证据路径；本次这些路径尚未生成，绝不表示存在截图或通过结果。建议交接时将 `M` 替换为实际证据根目录。

## 自动化门禁

在仓库根目录执行，npm 使用以下 Node 24 路径：

```sh
export PATH=/Users/bdxinhan/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin:$PATH
npm test
npm run build:budget
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc
npm run tauri -- build --bundles app,dmg
```

| 检查 | 状态 | 本次结果 | 证据 |
| --- | --- | --- | --- |
| `npm test` | 通过 | exit 0；22 个文件，92 passed，0 failed | E1 |
| `npm run build:budget` | 通过 | exit 0；vue-tsc、Vite 和预算通过，Monaco assets 4.51 MB | E2 |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 通过 | exit 0；92 passed，0 failed，2 ignored（60 lib + 32 integration；doc tests 0） | E3 |
| 完整 `cargo check --target x86_64-pc-windows-msvc` | 失败 | exit 101；`ring 0.17.14` 缺 `assert.h`，`aws-lc-sys 0.46.0` 缺 `windows.h` | E4 |
| Windows runner 全量测试、完整应用检查及 NSIS build | 未执行 | 没有 Windows 主机，未触发 CI，未生成 `.exe` | 无本次证据 |
| macOS app/DMG build | 通过 | exit 0；release 编译和两个 bundle 完成，app 27.03 MiB / DMG 14.56 MiB，x86_64 | E5 |
| macOS 产物校验 | 通过 | app 可执行文件存在，Info.plist 有效，`file` 确认为 x86_64；`hdiutil verify` exit 0，DMG checksum VALID | E6 |
| 既有性能基准及 60 秒压力测试 | 未执行 | 默认全量测试中的 2 个 ignored 项，未使用 `--ignored` | E3 |

Vite 仍提示已有 Monaco chunk 超过默认 500 kB；项目自己的 bundle 预算退出码为 0。

### Windows 检查边界

完整应用跨目标检查实际执行过且失败，原因是 macOS 环境缺少 Windows SDK/MSVC C 头文件和对应 C 工具链；此结果不能写成整应用 Windows 编译通过，也不足以认定 Windows 业务源码有编译错误。

前序 Tasks 2–5 已使用轻量 harness 通过 `#[path]` 直接引用实际 Windows 生产源文件，在 `x86_64-pc-windows-msvc` target 做类型检查。来源为本地 `.superpowers/sdd/2026-10-23-windows-compatibility/task-{2,3,4,5}-report.md`：

| 来源 | 实际检查范围 | 已有 harness 路径 |
| --- | --- | --- |
| Task 2 | `secrets.rs` DPAPI 包装、存储与 Windows-only 测试源码 | `/tmp/netcat-dpapi-check.vLAd9Z/Cargo.toml` |
| Task 3 | `certificates.rs` 当前用户物理 Root 证书存储源码 | `/tmp/netcat-certificates-check.nU5Z4b/Cargo.toml` |
| Task 4 | `system_proxy.rs` HKCU/WinINet 适配器源码 | `/tmp/netcat-system-proxy-check.JAdlI0/Cargo.toml` |
| Task 5 | 实际平台装配、全部 Windows 模块、模型和共享协调器 | `/tmp/netcat-platform-check.BQXDrg/Cargo.toml` |

这些是前序任务的类型检查证据，本次未重复执行；harness 使用 Store/AppError 等类型 shim，没有执行 DPAPI、证书、注册表、WinINet、版本或端口原生 API。macOS 全量测试中名为 `platform::windows::*` 的测试使用可移植逻辑及 fake 边界，同样不能算 Windows 原生 API 运行验收。

## 平台与安装包身份

| 平台 | 实际系统版本 | 安装包与 SHA-256 | 安装/运行验收状态 |
| --- | --- | --- | --- |
| Windows 10 22H2 x86_64 | 未执行：无主机，未采集 build 号 | 未执行：无 NSIS 包，无 SHA-256 | 未执行 |
| Windows 11 x86_64 | 未执行：无主机，未采集 edition/build 号 | 未执行：无 NSIS 包，无 SHA-256 | 未执行 |
| macOS（arm64 宿主 / x86_64 Rust target） | 26.2，25C56（E0、E0a） | `NetCat_0.3.0_x64.dmg`，SHA-256 见下表（E6） | 未执行 |

本次由上述基线本地构建，产物位于被 Git 忽略的 `src-tauri/target/release/bundle/`：

| 产物路径（仓库相对） | SHA-256 |
| --- | --- |
| `src-tauri/target/release/bundle/dmg/NetCat_0.3.0_x64.dmg` | `1e37a385e1e6f10c8945f8efcd63b7c4efd1b155edea43eccbafa49c35c0e067` |
| `src-tauri/target/release/bundle/macos/NetCat.app/Contents/MacOS/netcat` | `3aba2c778edf5da19a53097ec8161cea48ce7890c2da7646d1ea1bd6958c9b4e` |

macOS `.app` 为目录，以上第二个 SHA-256 仅代表可执行文件，不能代表完整 app bundle；DMG 则是可校验的完整安装介质。构建产物身份与安装运行验收独立，本次未启动 app、安装 CA 或修改系统代理/Keychain。

## 手工验收清单

每次验收先记录实际系统版本、代码 SHA、安装包来源和 SHA-256、WebView2 情况以及验收人。Windows 10 22H2 / Windows 11 必须分别填写；CI 的 `windows-2022` 不替代这两个系统。仅在执行并存档证据后把对应状态改为“通过”或“失败”。

| 项目 | 操作与通过条件 | Windows 10 22H2 | Windows 11 | macOS 26.2 | 待采集证据路径 |
| --- | --- | --- | --- | --- | --- |
| 系统版本 | 记录实际 edition、build 与 CPU 架构；macOS 已采集 E0 | 未执行 | 未执行 | 通过（E0） | `M/<平台>/environment/` |
| 安装包 SHA-256 | 对本次待验收安装包计算 SHA-256，和来源/代码版本绑定 | 未执行 | 未执行 | 通过（仅构建产物，E6） | `M/<平台>/artifact/`；本次 macOS 见 E6 |
| 安装与首次启动 | 标准用户安装并启动；Windows 确认当前用户 NSIS 范围，分别记录 WebView2 预装/下载分支；macOS 从 DMG 复制 app 后启动 | 未执行 | 未执行 | 未执行 | `M/<平台>/install/` |
| HTTP 抓包 | 经本机代理访问可控 HTTP 上游，核对请求与响应、状态、正文和列表详情 | 未执行 | 未执行 | 未执行 | `M/<平台>/http/` |
| HTTPS 抓包 | 用户显式完成 CA 信任后经代理访问可控 HTTPS 上游，确认无证书错误且可查看解密流量 | 未执行 | 未执行 | 未执行 | `M/<平台>/https/` |
| Mock | 建立、启用、编辑及禁用规则，验证优先级、命中结果、状态/正文和记录 | 未执行 | 未执行 | 未执行 | `M/<平台>/mock/` |
| 重发 | 编辑请求后重发，核对上游收到的修改与来源关联；停止代理后仍可重发 | 未执行 | 未执行 | 未执行 | `M/<平台>/replay/` |
| CA 私钥保护 | Windows 使用同用户保存/重读，文件不是 PEM，损坏时错误不泄露正文；macOS 核对现有 Keychain 写入/读取 | 未执行 | 未执行 | 未执行 | `M/<平台>/private-key/` |
| CA 安装信任 | 仅由用户点击触发；Windows 检查 CurrentUser 物理 Root，不能把继承的同证书误当本地安装；macOS 按现有导出/Keychain 流程验证 | 未执行 | 未执行 | 未执行 | `M/<平台>/ca-install/` |
| CA 移除 | 按 DER SHA-256 仅移除本产品当前用户证书；保留同名异指纹和继承证书；macOS 手动移除后复核 Keychain | 未执行 | 未执行 | 未执行 | `M/<平台>/ca-remove/` |
| 系统代理接管与正常恢复 | 保存原设置，开启/关闭代理、正常退出；Windows 核对 HKCU 五字段（含缺失/空值、bypass/PAC/auto-detect）及 WinINet 生效；macOS 核对原网络服务 HTTP/HTTPS 配置恢复 | 未执行 | 未执行 | 未执行 | `M/<平台>/proxy-restore/` |
| 端口占用提示 | 由已知进程占用目标端口后启动代理，核对进程名/PID；无进程名权限时退化 PID，不泄露完整路径 | 未执行 | 未执行 | 未执行 | `M/<平台>/port-conflict/` |
| 异常退出恢复 | 在受控账户启用代理后强制终止，再启动；无外部修改时恢复原设置，失败时保留可重试快照 | 未执行 | 未执行 | 未执行 | `M/<平台>/crash-recovery/` |
| 用户修改代理冲突 | 接管后在系统设置改代理，再停止/重启；保留用户设置、提示冲突且保留快照；包含用户改回原值的情况 | 未执行 | 未执行 | 未执行 | `M/<平台>/proxy-conflict/` |
| 诊断包敏感信息扫描 | 使用真实验收流量导出诊断包，按下节扫描并逐项人工判断字段值 | 未执行 | 未执行 | 未执行 | `M/<平台>/diagnostics/` |
| 卸载与残留检查 | 先停止并恢复代理、移除本产品 CA；卸载后复核代理/证书不受损、应用移除、用户数据按说明处理 | 未执行 | 未执行 | 未执行 | `M/<平台>/uninstall/` |

## 真实诊断包扫描

本次三个平台均没有产生真实手工验收诊断包，扫描状态全部为“未执行”。自动化诊断脱敏测试通过不能代替该步骤；没有制造样本包或报告“零命中”。

对每个平台实际导出的 ZIP 记录来源、SHA-256 和解压目录后执行：

```sh
rg -n -i "authorization|cookie|token|private key|BEGIN (RSA |EC )?PRIVATE KEY|dpapi" <diagnostic-directory>
```

记录扫描命令、退出码和全部命中。`rg` exit 1 表示无匹配，exit 2 表示扫描出错，不能混淆；exit 0 的每个命中均须人工确认。允许的错误字段名须证明其值不含凭据；请求/响应正文、Cookie、Authorization、Token、私钥和 DPAPI 密文均不得出现。原始诊断包、扫描结果与人工确认放在对应 `M/<平台>/diagnostics/`，敏感命中未经处理不能提交到仓库。

## 发布前未完成项

- [ ] 在真实 Windows runner 完成完整应用 target 检查、全量测试与 NSIS 构建，保存 `.exe` SHA-256 和运行日志。
- [ ] 在 Windows 10 22H2 与 Windows 11 分别执行全部手工清单，保留系统 build 号、安装/卸载、当前用户原生 API 和代理恢复证据。
- [ ] 在 macOS 使用本次 app/DMG 执行 Keychain、代理恢复、现有功能与卸载回归。
- [ ] 扫描并人工复核三个平台真实验收产生的诊断包。

以上未执行项关闭前，不把本记录作为三平台发布验收通过依据。
