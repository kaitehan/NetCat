# 贡献指南

感谢你关注 NetCat（奈特猫）。项目欢迎缺陷修复、功能改进、测试、文档和平台兼容性方面的贡献。

## 开始之前

- 仅针对你拥有或已明确获准测试的设备、应用和网络进行开发与验证。
- 安全漏洞不要提交到公开 Issue，请遵循 [安全策略](SECURITY.md)。
- 较大的功能或行为变更建议先创建 Issue，说明问题、使用场景、范围和验收方式，避免重复工作。
- Windows 相关改动请注明已验证的平台、Windows 版本和工具链；无法执行的实机测试也应明确标记为未执行。

## 开发环境

基础要求：

- Node.js 22 或更高版本
- Rust 1.85 或更高版本
- macOS 12+，或 Windows 10 22H2 / Windows 11 x86_64

安装依赖并启动前端预览：

```bash
npm install
npm run dev
```

启动真实桌面应用：

```bash
npm run tauri dev
```

平台依赖、Windows MSVC 工具链和 WebView2 要求请参阅 [README](README.md#开发环境)。

## 提交改动

1. 从最新的 `main` 创建功能分支。
2. 保持改动聚焦，不在同一提交中混入无关重构。
3. 新功能和缺陷修复应包含能够复现行为的自动化测试。
4. Vue 代码使用 Composition API、`<script setup lang="ts">`、props 向下和事件向上。
5. CSS 保持多行格式，每条属性单独占一行。
6. 提交信息使用 Conventional Commits，例如 `fix(proxy): 修复系统代理恢复冲突`。

## 验证

提交 Pull Request 前至少运行：

```bash
npm test
npm run build:budget
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

涉及桌面运行时或打包的改动，还应在对应平台运行适用的 Tauri 构建。Windows NSIS 构建命令为：

```powershell
npm run tauri:build:windows
```

60 秒压力测试耗时较长，默认被忽略。修改代理热路径、数据库写入或流量持久化时应单独运行：

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test proxy_stress sustains_200_qps_for_60_seconds_and_keeps_5000_rows -- --ignored --exact
```

## Pull Request 内容

请在 Pull Request 中说明：

- 解决的问题和用户影响
- 主要实现方式及关键取舍
- 已执行的测试、构建和手工验证
- 未执行的验证及原因
- 涉及界面时提供变更前后截图
- 涉及系统代理、证书或私钥时说明失败恢复和安全边界

维护者可能要求缩小范围、补充测试或调整接口后再合并。
