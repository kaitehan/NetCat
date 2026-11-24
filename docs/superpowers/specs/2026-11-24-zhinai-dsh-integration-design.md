# 智奈集成 DSH Harness 设计

## 背景

NetCat 当前使用 Vue 3、Tauri 2 和 Rust 构建桌面应用。智奈已具备对话面板、历史列表和本地消息存储，但尚未连接模型或 Agent Runtime。现有消息仅包含用户输入，并保存在浏览器 `localStorage` 中。

本设计将 DeepSeek Harness（DSH）作为随 NetCat 安装的本地 Agent Runtime。智奈继续使用现有 Vue 界面，Rust 负责可信控制、NetCat 业务访问和进程生命周期，TypeScript Bridge 使用 DSH 官方 SDK 驱动 Runtime。用户无需安装 Node.js、Python 或 DSH。

DSH 当前属于预稳定 API。集成必须将版本变化限制在 Bridge、Profile 和专用插件内，避免 DSH 协议进入 NetCat Rust 核心或 Vue 界面。

## 目标

首版提供以下能力：

- 用户在智奈中粘贴 cURL，获得默认不联网的结构化分析。
- 用户从抓包详情发起分析，智奈读取脱敏的请求、响应、耗时和诊断信息。
- 智奈比较相似请求，给出有证据和置信度的接口异常原因。
- 智奈生成 Mock 或 QuickJS 脚本草稿，用户确认后再保存并启用。
- 用户明确点击“执行诊断”后，NetCat 使用原始 cURL 发起一次受控请求。
- 智奈对话由 DSH Session 持久化，重启 NetCat 后可以继续。
- 用户在“设置 → 智奈配置”中显式选择 DeepSeek 或 OpenAI，并管理各自 API Key。
- macOS Apple Silicon 与 Windows x64 安装包内置完整 Runtime。

## 非目标

首版不包含：

- 模型逐 Token 文本流。界面展示真实活动状态，最终回复一次性出现。
- 自动在 DeepSeek 和 OpenAI 之间故障切换。
- 多个会话同时运行 Agent Turn。
- 通用 Shell、文件系统、浏览器或计算机控制工具。
- Agent 自动修改系统代理、CA、已有 Mock 或已有 QuickJS 脚本。
- 未经用户确认自动执行 cURL、重放或启用规则。
- macOS Intel、Windows ARM 或 Linux Runtime。
- 迁移现有仅包含用户消息的 `localStorage` 历史。

## 已确认决策

- 采用 TypeScript Bridge + DSH Runtime + NetCat 专用插件。
- cURL 默认只解析；用户点击“执行诊断”后才允许联网。
- API Key 由用户提供，并由 NetCat 的 Rust 平台存储保护。
- Runtime 完整内置，不依赖用户系统中的 Node.js、Python 或 DSH。
- 首期支持 macOS Apple Silicon 和 Windows x64。
- 不迁移旧智奈历史，启用新的 DSH Session。
- 首版显示分析状态和工具状态，完成后一次性显示回复。
- 用户显式选择 DeepSeek 或 OpenAI，不自动切换服务商。

## 总体架构

```text
┌──────────────────── NetCat 主进程 ────────────────────┐
│ Vue 智奈 UI                                           │
│   ↓ Tauri command / event                             │
│ Rust AgentSupervisor                                  │
│   ├─ Session 映射与运行状态                            │
│   ├─ API Key 与平台安全存储                            │
│   ├─ 审批、取消、超时与进程恢复                         │
│   └─ NetCatToolServer                                 │
│       └─ 抓包、Mock、重放、诊断业务接口                  │
└──────────────────┬────────────────────▲───────────────┘
                   │ stdio JSON-RPC     │ 本地私有 IPC
                   ▼                    │
┌───────────────────────────────────────────────────────┐
│ NetCat Harness Bridge                                 │
│   └─ @deepseek-ai/dsh-sdk-client                      │
│                    ↓                                  │
│ DSH Runtime                                           │
│   ├─ Session / Agent Loop / LLM                       │
│   ├─ NetCat 专用 Profile                              │
│   └─ NetCat Tool Plugin ──────────────────────────────┘
└───────────────────────────────────────────────────────┘
```

### Vue 智奈 UI

Vue 只负责对话和操作呈现：用户输入、活动状态、最终回复、工具卡片、Mock 预览、确认操作和错误提示。Vue 不接触 API Key，不直接启动 DSH，也不访问工具 IPC。

### Rust AgentSupervisor

`AgentSupervisor` 是应用内唯一可信控制者，负责：

- 启动、监控和关闭 Bridge 及其 DSH 子进程。
- 将 NetCat 会话 ID 映射到 DSH Session ID。
- 将 Bridge 事件转换为 Tauri 事件。
- 管理当前活动 Turn、停止请求和恢复状态。
- 处理工具审批、一次性联网授权和参数复核。
- 在应用退出时确认整个子进程树已经结束。

### TypeScript Bridge

Bridge 使用官方 `@deepseek-ai/dsh-sdk-client`，向 Rust 暴露 NetCat 自有的逐行 JSON-RPC 协议。Bridge 负责 SDK 初始化、Session 句柄、通知转换、审批回答和 DSH Runtime 关闭。DSH SDK 升级造成的接口变化不得传播到 Vue 或 Rust 业务模块。

### DSH Runtime 与专用 Profile

Runtime 通过支持 SDK 协议的专用 `netcat-agent` Profile 启动。Profile 只启用模型、Session、审批、NetCat 凭据 Provider、Session Projection 和 NetCat 工具，不启用通用 Shell、文件、浏览器、计算机操作或插件安装能力。

Runtime 的 Harness Home 位于 NetCat 应用数据目录。Agent 工作目录使用该目录下的隔离空目录，不使用用户工程目录。

### NetCat Tool Plugin

专用插件声明 Agent 可调用的 NetCat 领域工具。插件不直接打开 NetCat SQLite，不读取平台凭据，也不修改代理状态。所有业务操作通过私有 IPC 请求 Rust，由 Rust 复用现有 Store、Mock、Replay 和诊断服务完成。

## 进程与通信

进程关系固定为：

```text
NetCat Rust → Node Bridge → DSH Runtime
```

Bridge 退出时必须关闭 DSH。Rust 检测到 Bridge 或 DSH 异常退出时，先终止残留进程树，再决定是否恢复。

Rust 与 Bridge 使用 stdin/stdout 上的逐行 JSON-RPC。DSH 插件与 Rust 使用本地私有双向 RPC：

- macOS 使用 Unix Domain Socket，并将文件权限限制为当前用户。
- Windows 使用 Named Pipe，并配置当前用户 ACL。
- 每次启动生成新的随机认证令牌。
- Rust 校验连接属于自己启动的进程树。
- IPC 地址和令牌只传给 Bridge 与 DSH 子进程，不进入日志或 Session。

工具 RPC 允许 DSH 插件请求 Rust 执行领域工具，也允许 Rust请求插件返回 Session Projection Snapshot。

## Bridge 协议

Rust 发出的主要请求：

| 方法 | 作用 |
|---|---|
| `runtime/initialize` | 初始化 Bridge 和 DSH Runtime |
| `session/create` | 创建一个新的智奈会话 |
| `session/resume` | 恢复已有 DSH Session |
| `session/run` | 向 Session 提交用户输入 |
| `session/snapshot` | 获取可用于 UI 的 Session 投影 |
| `session/stop` | 停止当前分析并恢复 Runtime |
| `runtime/shutdown` | 正常关闭 Runtime 和 Bridge |

Bridge 发布的事件：

- `runtime/status`
- `session/status`
- `session/activity`
- `tool/started`
- `tool/completed`
- `approval/requested`
- `session/completed`
- `session/failed`

每个操作消息携带 `requestId`、`conversationId`、`sessionId` 和 `operationId`。Rust 丢弃不属于当前 Operation 的迟到事件，避免停止后的结果污染新 Turn。

Bridge 协议在 `harness/bridge` 中进行运行时校验。无效消息使当前操作失败，但不得使 Rust 接受部分字段或执行工具。

## Session 与持久化

每个 NetCat 智奈会话一对一映射一个 DSH Session。

- DSH Session Log 是 Agent 对话和工具执行的事实来源。
- NetCat SQLite 保存会话列表、标题、DSH Session ID、更新时间和 UI 投影缓存。
- `localStorage` 不再保存智奈对话，只保留纯 UI 偏好。

NetCat 专用插件注册 Session Projection，折叠用户消息、Assistant 最终消息、工具调用、工具结果、审批结果和 Turn 状态。应用异常退出后，Rust 从 DSH Snapshot 重建 NetCat UI 缓存。

发送消息的顺序：

1. Rust 将用户输入和 `operationId` 写入 SQLite。
2. Bridge 将输入提交给 DSH。
3. DSH 在 Session 中记录被接收的消息。
4. Bridge 返回完成结果和工具事件。
5. Rust 在一个事务中更新 UI 投影并标记 Operation 完成。
6. 中途崩溃时，下次启动从 DSH Projection 重新对齐。

删除会话时，NetCat 先隐藏 UI 映射。首版保留 DSH Session 数据并延迟清理，避免误删导致不可恢复。

## 运行状态与停止

首版全局只允许一个活动 Turn。用户可以浏览其他历史会话，但活动 Turn 完成前不能从另一个会话发送新消息。

DSH SDK 当前没有独立的 Session Cancel 方法。用户点击停止时，Rust 关闭当前 DSH Runtime，回收进程树，启动新的 Runtime并恢复当前 Session。DSH 负责修复中断 Turn，智奈显示“上次分析已停止”。

状态机：

```text
stopped → starting → ready → running → ready
                     ↓         ↓
                  recovering ←─┘
                     ↓
                   error
```

Bridge 或 DSH 意外退出时，每次故障最多自动重启一次。连续失败、API Key 无效、Profile 配置错误和协议不兼容进入 `error`，不循环重启。UI 提供“重新启动智奈”和脱敏诊断信息。

## 首版回复体验

官方 SDK 当前提供持久化 Session 事件、工具状态和最终回复，不提供逐 Token Assistant 文本。首版展示真实活动状态：

```text
正在分析 cURL
→ 正在检索相似请求
→ 正在比较响应
→ 正在生成诊断
→ 显示最终回复
```

不得通过计时器伪造模型文本流。未来 SDK 提供增量通知时，可以增加 `assistant/delta` 事件，不改变 Session 和 UI 数据模型。

## cURL 输入模型

cURL 不作为未解析的普通 Prompt 直接发送给模型。Rust 使用结构化解析器生成：

```text
CurlRequest
├─ method / URL / query
├─ headers
├─ cookies
├─ body 类型与内容
├─ timeout / redirect / TLS 参数
└─ sensitiveFields
```

解析过程不得执行 shell 或调用系统 `curl`。解析失败返回具体位置和可修复提示。

Authorization、Cookie、Token、密码等原值只保留在 Rust 当前操作的内存中。DSH Session 只记录脱敏投影。模型可以知道敏感字段是否存在、认证类型和长度，但看不到原值。原始数据在请求完成、取消或应用退出后清除。恢复历史会话时只能继续分析脱敏内容，不能重新执行旧 cURL。

## 数据脱敏

默认脱敏规则：

- Header 名称可见，敏感 Header 值替换为占位符。
- JSON Body 对 `token`、`password`、`secret`、`key` 等字段递归脱敏。
- Body 超过限制时只发送摘要、类型和受限片段。
- 二进制 Body 不发送给模型，只发送 MIME、长度和哈希。
- 原始 Authorization、Cookie、Token 和密码不进入日志、SQLite、诊断包或 DSH Session。
- 用户可以在请求摘要中查看“将发送给模型的内容”。

首次启用智奈时，界面说明脱敏后的输入会发送给用户选定的模型提供商。

## NetCat 工具

| 工具 | 用途 | 权限 |
|---|---|---|
| `netcat_parse_curl` | 读取 Rust 已解析的脱敏请求 | 自动允许 |
| `netcat_get_exchange` | 获取指定抓包的脱敏详情 | 自动允许 |
| `netcat_find_similar_exchanges` | 查找有限数量的相似请求 | 自动允许 |
| `netcat_get_diagnostics` | 获取代理、TLS、证书和网络诊断 | 自动允许 |
| `netcat_compare_exchanges` | 比较成功与失败请求 | 自动允许 |
| `netcat_create_mock_draft` | 创建结构化 Mock 草稿 | 仅创建草稿 |
| `netcat_create_script_draft` | 创建 QuickJS 脚本草稿 | 仅创建草稿 |
| `netcat_execute_curl` | 实际发送原始请求 | 单次用户授权 |

保存并启用 Mock、保存并启用 QuickJS 脚本以及发起普通重放不作为 Agent 可直接执行的工具。用户在预览卡片中确认后，Vue 通过现有 Tauri 命令调用 Rust。

工具只能接收资源 ID 或一次性请求句柄，不接受任意本地路径、shell 命令或由模型提供的凭据。

## cURL 异常分析流程

```text
用户粘贴 cURL
  → Rust 解析并脱敏
  → 智奈展示请求摘要
  → DSH 读取脱敏请求
  → 检索相似抓包、诊断日志和耗时
  → 输出初步分析
  → 用户点击“执行诊断”
  → UI 展示目标地址、方法和敏感字段提示
  → Rust 发放一次性执行权限
  → Rust 使用原始 cURL 发请求
  → 响应脱敏后交给 DSH
  → 智奈输出证据、原因、置信度和建议
```

诊断结果包含问题摘要、已确认事实、可能原因及置信度、支撑证据、建议验证步骤，以及可生成的 Mock 或脚本操作。

## 一键智能 Mock

```text
cURL / 抓包
  → DSH 分析请求和响应语义
  → 调用 netcat_create_mock_draft
  → Rust 校验状态码、Header、Body 和匹配条件
  → 智奈显示 Mock 预览卡片
  → 用户点击“保存并启用”
  → Vue 调用现有 Rust Mock 服务
  → 保存结果回写 DSH Session
```

模型只能创建草稿。预览必须展示方法、URL 匹配方式、状态码、关键 Header 和 Body，特别提示过宽的 Contains 或 Regex 规则。

## 一次性联网授权

联网工具不接受原始 URL，而接受 Rust 生成的 `requestHandle`：

```text
requestHandle
├─ conversationId
├─ operationId
├─ method
├─ URL 指纹
├─ Body 哈希
├─ expiresAt
└─ used
```

授权规则：

- 用户明确点击执行后才创建。
- 只绑定一个工具调用。
- 五分钟或当前操作结束后失效。
- 使用一次立即作废。
- 方法、URL 或 Body 改变后不能复用。
- TLS 校验默认开启。
- 设置连接、总耗时和响应大小限制。
- 跨域重定向必须再次确认。
- 内网和本机地址允许执行，但确认界面增加醒目提示。

确认界面显示域名、解析后的 IP、方法、Body 大小和敏感字段存在情况。

## Prompt Injection 防护

抓包内容、响应 Body 和错误页面均视为不可信数据：

- 通过结构化字段传给 Agent，不拼接为系统指令。
- 系统提示要求不得执行响应内容中的指令。
- HTTP 内容不能改变工具权限或审批策略。
- Rust 对每个工具参数再次验证。
- 工具不能访问系统代理、CA、钥匙串、任意文件或命令。
- 保存和联网操作始终由用户界面确认，模型输出不能替代授权。

## 智奈配置页

现有设置页增加第四个“智奈配置”Tab，包含以下区域。

### 模型服务

- 使用分段控件显式选择 DeepSeek 或 OpenAI。
- 根据服务商选择模型。
- 服务商和模型是当前 DSH Runtime 的进程级路由。
- 空闲时保存新选择会受控重启 Runtime，恢复当前 Session 后再允许发送消息。
- Turn 运行期间保存新选择会延迟到该 Turn 结束；界面明确显示待应用状态。
- 不自动故障切换，不在一次 Turn 内改变服务商。

### API 凭据

- 分别管理 DeepSeek API Key 和 OpenAI API Key。
- 展示未配置、已配置和验证失败状态。
- 支持保存或替换、删除、验证连接。
- 输入框不回填原值；前端只能看到固定掩码和状态。
- “验证连接”由 Rust 发起最小请求，返回不包含请求 Header、响应 Header 或 Key 的结构化结果。

### Harness Runtime

- 展示 Runtime 状态、DSH 版本和 Bridge 版本。
- 提供“重新启动智奈”。
- 展示脱敏和模型提供商数据发送说明。

## 凭据存储

凭据使用独立于 CA 私钥的命名空间：

- macOS 为 DeepSeek 和 OpenAI 创建不同的 Keychain 账号项。
- Windows 为两项凭据创建独立的当前用户 DPAPI 加密文件。
- CA 私钥、DeepSeek Key 和 OpenAI Key 使用不同标识和文件，互不覆盖。
- SQLite 只保存当前服务商、模型 ID 和配置状态，不保存 Key。
- Vue 只调用 `describe`、`save`、`delete` 和 `validate`，不存在读取 Key 的 Tauri 命令。

NetCat DSH 凭据 Provider 根据当前模型路由解析：

```text
DeepSeek → DEEPSEEK_API_KEY → Rust 安全存储
OpenAI   → OPENAI_API_KEY   → Rust 安全存储
```

Provider 通过私有 IPC 按模型请求解析凭据，不把 Key 写入 DSH 配置、环境变量、文件或 Session。DSH 模型适配器在发送请求时会短暂持有 Key，这是完成请求所必需的；Bridge 不读取 Key。

删除当前服务商的 Key 后，下一次 Turn 明确要求重新配置，不自动切换另一服务商。

## 前端交互

智奈沿用现有入口和侧边面板。消息区域增加以下稳定状态：

- Runtime 启动中
- 等待模型
- 正在调用只读工具
- 等待用户确认
- 正在执行联网诊断
- 已完成
- 已停止
- 可重试错误
- 需要配置凭据

工具结果使用独立卡片展示。Mock 和脚本卡片提供预览、保存并启用、取消操作。错误保留用户输入和已有内容，不因失败清空会话。

现有 `useSupportChat` 改为异步 Session 客户端，不再直接写 `localStorage`。UI 组件不得直接解析 DSH 原始事件，而使用 Rust 发布的 NetCat 稳定事件模型。

## 建议目录

```text
my-charles/
├─ harness/
│  ├─ bridge/
│  │  ├─ src/index.ts
│  │  ├─ src/protocol.ts
│  │  └─ tests/
│  ├─ plugin-netcat/
│  │  ├─ src/index.ts
│  │  ├─ src/tools/
│  │  ├─ src/credentials/
│  │  └─ tests/
│  ├─ profile/
│  │  ├─ package.json
│  │  └─ cordis.patch.yml
│  └─ package.json
├─ src-tauri/src/assistant/
│  ├─ mod.rs
│  ├─ supervisor.rs
│  ├─ bridge_protocol.rs
│  ├─ tool_server.rs
│  ├─ credentials.rs
│  ├─ curl_parser.rs
│  ├─ redaction.rs
│  └─ models.rs
├─ src/features/support/
│  ├─ SupportChatPanel.vue
│  ├─ useSupportChat.ts
│  ├─ AssistantMessage.vue
│  ├─ AssistantActivity.vue
│  └─ MockDraftCard.vue
└─ src/features/settings/
   └─ AssistantSettings.vue
```

## Runtime 打包

每个平台的安装资源包含：

```text
resources/harness/
├─ bin/node
├─ bridge/index.mjs
├─ runtime/node_modules/
├─ profile-template/
│  ├─ package.json
│  └─ cordis.patch.yml
└─ manifest.json
```

构建要求：

- 选择并固定一个精确 DSH 版本；CLI、SDK、Profile 依赖和插件依赖保持同版本。
- 固定满足 DSH 要求的 Node 版本。
- macOS ARM64 与 Windows x64 分别在对应平台构建 Runtime。
- `manifest.json` 记录版本、平台、架构和文件哈希。
- Rust 启动前校验 Runtime、Bridge 和插件版本相容。
- Node、原生依赖和应用安装包参与平台签名。
- NetCat 更新时整体更新 Runtime，不允许单独漂移。
- Runtime 不从用户 PATH、全局 npm 或用户 DSH Home 解析依赖。
- 首次启动将签名资源中的 Profile 清单和 patch 初始化到 NetCat 专属 Harness Home；版本升级以资源版本为准更新产品拥有的 Profile 文件，不覆盖 Session 数据。
- 智奈首次打开时懒启动 Runtime，之后保持到应用退出。

## 测试策略

### Rust 单元测试

- cURL 解析和错误定位。
- Header、JSON、文本和二进制 Body 脱敏。
- 一次性授权句柄、防重放和过期。
- Bridge 生命周期和进程树回收。
- Socket/Named Pipe 鉴权。
- DeepSeek/OpenAI 凭据保存、删除和描述。
- Mock 与脚本草稿结构验证。

### Bridge 与插件测试

- JSON-RPC 请求、事件关联和迟到事件丢弃。
- SDK 启动、恢复、失败和关闭。
- 工具输入和输出校验。
- 凭据按模型请求解析。
- Profile 未提供 Shell、文件和浏览器工具。
- DSH Session Projection 重建 UI Snapshot。

### Vue 测试

- Runtime 启动、活动、失败和最终回复状态。
- Mock 预览、确认保存和取消。
- 执行诊断的一次性确认。
- 智奈配置 Tab、Provider 切换和 Key 状态。
- Runtime 崩溃后的恢复提示。
- 长 cURL、长错误信息和窄窗口布局。

### 端到端验收

1. 粘贴带 Authorization 的 cURL，确认日志、SQLite、诊断包和 DSH Session 均无原始 Key。
2. 离线分析 cURL，确认没有目标网络请求。
3. 点击执行诊断，确认只请求原始目标一次。
4. 生成 Mock 草稿，确认保存前规则不生效。
5. 对比成功和失败抓包，输出证据化诊断。
6. 分析过程中结束 DSH，确认恢复 Session。
7. 删除当前 Provider Key，确认下一次 Turn 明确失败。
8. 验证 DeepSeek 与 OpenAI 的显式切换，不发生自动回退。
9. 在 macOS ARM64 和 Windows x64 安装包中冷启动智奈。

测试使用模拟模型和本地固定 HTTP 服务，不依赖真实 API Key。真实 Provider 冒烟测试作为发布环境的显式检查，不进入普通单元测试。

## 实施阶段

### 阶段 0：打包可行性验证

- 在两个目标平台运行内置 Node 和 DSH SDK。
- 验证签名、安装路径、进程回收和安装包体积。
- 验证 DSH 精确版本可从生产安装资源启动。
- 未通过该阶段不得开始完整 UI 和工具开发。

### 阶段 1：Runtime 基础

- 实现 Bridge、Supervisor、专用 Profile 和凭据 Provider。
- 实现 Session 创建、恢复、状态和最终回复。
- 将智奈历史从 `localStorage` 切换为 DSH Session。
- 增加“智奈配置”Tab 与 DeepSeek/OpenAI 凭据管理。

### 阶段 2：cURL 分析

- 实现 Rust cURL 解析、脱敏投影和请求摘要。
- 实现只读诊断工具和相似抓包检索。
- 智奈展示分析活动和最终诊断报告。

### 阶段 3：智能 Mock

- 实现结构化 Mock 草稿工具。
- 实现 Mock 预览、验证、保存和启用。
- 增加 QuickJS 脚本草稿。

### 阶段 4：联网诊断

- 实现一次性授权句柄。
- 实现执行前目标确认。
- 执行请求并将脱敏结果送回同一 Session 分析。

### 阶段 5：发布加固

- 完成崩溃恢复、诊断包脱敏和资源哈希校验。
- 验证 macOS 与 Windows 安装包及签名。
- 建立 DSH 升级兼容性检查和完整回归测试。

## 首版验收标准

首版完成时，用户能够：

- 在智奈中粘贴 cURL 并获得默认离线分析。
- 从抓包详情直接让智奈诊断。
- 查看基于事实、证据和置信度的异常原因。
- 生成、预览并确认启用 Mock。
- 明确授权后执行一次联网诊断。
- 在设置中选择 DeepSeek 或 OpenAI 并管理各自 Key。
- 重启 NetCat 后继续已有智奈会话。
- 在 macOS Apple Silicon 和 Windows x64 上无额外环境依赖地使用智奈。

上述能力必须同时满足：原始凭据不进入模型上下文或持久化记录、模型不能绕过审批执行写操作、Runtime 退出后不存在遗留子进程。

## 主要风险与控制

| 风险 | 控制措施 |
|---|---|
| DSH 预稳定 API 变化 | 固定精确版本；变化收敛在 Bridge/Profile；升级运行兼容测试 |
| 安装包体积和签名复杂度 | 阶段 0 先验证两平台 Runtime 产物 |
| DSH 与 UI 历史分叉 | DSH Session 为事实来源；Projection 重建 UI 缓存 |
| cURL 泄露凭据 | Rust 解析；原值仅在内存；模型只见脱敏投影 |
| Prompt Injection 触发操作 | 结构化输入、固定工具权限、Rust 复核和用户确认 |
| Runtime 崩溃留下进程 | 固定父子关系、进程树清理、一次自动恢复 |
| 模型服务商费用或数据目标不明确 | 用户显式选择服务商；不自动回退；发送前展示数据说明 |
