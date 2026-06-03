# Qwen Code settings.json 配置参考

> 基于 Qwen Code v0.16.2，用于 AgentBox 工具箱的配置解析、生成和 merge 开发参考。

## 1. 配置层级与优先级

| 优先级（低→高） | 来源 | 路径 |
|---|---|---|
| 1 | 默认值 | 硬编码 |
| 2 | 系统默认文件 | Linux: `/etc/qwen-code/system-defaults.json` / Windows: `C:\ProgramData\qwen-code\system-defaults.json` |
| 3 | 用户设置 | `~/.qwen/settings.json` |
| 4 | 项目设置 | `<project>/.qwen/settings.json` |
| 5 | 系统设置 | Linux: `/etc/qwen-code/settings.json` / Windows: `C:\ProgramData\qwen-code\settings.json` |
| 6 | 环境变量 | `$VAR` / `${VAR}` 引用 |
| 7 | 命令行参数 | `--flag` |

**merge 规则要点：**
- `modelProviders` 项目级整体覆盖用户级（REPLACE 策略，不做字段合并）
- `generationConfig` 对 Provider Model 是不可渗透的原子包，不从低层继承
- 字符串值支持 `$VAR` / `${VAR}` 环境变量插值
- 配置文件支持 JSONC 格式（`//` 和 `/* */` 注释）

---

## 2. 顶层结构总览

```
settings.json
├── proxy                        (string)
├── plansDirectory               (string)
├── fastModel                    (string)
├── general                      (object)
├── output                       (object)
├── model                        (object)    ← 核心
├── modelProviders               (object)    ← 核心
├── env                          (object)    ← 核心
├── mcpServers                   (object)    ← 核心
├── mcp                          (object)
├── hooks                        (object)    ← 核心
├── permissions                  (object)    ← 核心
├── tools                        (object)
├── context                      (object)
├── memory                       (object)
├── ui                           (object)
├── ide                          (object)
├── privacy                      (object)
├── security                     (object)
├── slashCommands                (object)
├── telemetry                    (object)
├── advanced                     (object)
├── codingPlan                   (object)
└── experimental                 (object)
```

---

## 3. 各模块详细字段

### 3.1 general — 通用设置

| 字段 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `general.preferredEditor` | string | `undefined` | 首选编辑器 |
| `general.vimMode` | boolean | `false` | Vim 键绑定 |
| `general.enableAutoUpdate` | boolean | `true` | 自动更新 |
| `general.showSessionRecap` | boolean | `false` | 回归时显示摘要 |
| `general.sessionRecapAwayThresholdMinutes` | number | `5` | 离开多少分钟后触发 recap |
| `general.gitCoAuthor.commit` | boolean | `true` | commit 添加 AI 署名 |
| `general.gitCoAuthor.pr` | boolean | `true` | PR 添加 AI 署名 |
| `general.checkpointing.enabled` | boolean | `false` | 会话检查点 |
| `general.defaultFileEncoding` | string | `"utf-8"` | 默认文件编码 |

### 3.2 model — 模型配置

| 字段 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `model.name` | string | `undefined` | 当前使用的模型名 |
| `model.maxSessionTurns` | number | `-1` | 最大会话轮次，-1 无限 |
| `model.maxWallTimeSeconds` | number | `-1` | 无头模式运行时间限制 |
| `model.maxToolCalls` | number | `-1` | 累计工具调用预算 |
| `model.skipNextSpeakerCheck` | boolean | `false` | 跳过下一位发言者检查 |
| `model.skipLoopDetection` | boolean | `true` | 跳过流式循环检测 |
| `model.skipStartupContext` | boolean | `false` | 跳过启动时工作区上下文 |
| `model.enableOpenAILogging` | boolean | `false` | OpenAI API 调用日志 |
| `model.openAILoggingDir` | string | `undefined` | API 日志目录 |

**model.generationConfig — 生成配置（原子包）：**

| 字段 | 类型 | 说明 |
|---|---|---|
| `timeout` | number | 请求超时（ms） |
| `maxRetries` | number | 最大重试次数 |
| `enableCacheControl` | boolean | 缓存控制 |
| `splitToolMedia` | boolean | 严格兼容服务器拆分媒体内容 |
| `contextWindowSize` | number | 覆盖上下文窗口大小 |
| `modalities` | object | `{ image, pdf, audio, video }` 覆盖输入模态 |
| `customHeaders` | object | 自定义 HTTP 头 |
| `extra_body` | object | 额外请求体参数（仅 openai/qwen-oauth） |
| `samplingParams` | object | 采样参数 |
| `samplingParams.temperature` | number | 温度 |
| `samplingParams.top_p` | number | Top-P |
| `samplingParams.max_tokens` | number | 最大输出 token（不设时自适应 8K→64K） |
| `samplingParams.presence_penalty` | number | 存在惩罚 |
| `samplingParams.frequency_penalty` | number | 频率惩罚 |
| `reasoning` | object/boolean | 推理配置 `{ effort: "low"|"medium"|"high"|"max", budget_tokens }` 或 `false` 禁用 |

### 3.3 modelProviders — 模型提供商（核心）

按 authType 分组的模型列表。每个 authType 下是数组。

**支持的 authType：**

| authType | SDK | 说明 |
|---|---|---|
| `openai` | `openai` | OpenAI 兼容 API（含 vLLM/Ollama/LM Studio/中转） |
| `anthropic` | `@anthropic-ai/sdk` | Anthropic Claude API |
| `gemini` | `@google/genai` | Google Gemini API |
| `qwen-oauth` | `openai`（自定义 provider） | 硬编码，不可覆盖 |

**ProviderEntry 结构：**

| 字段 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `id` | string | ✅ | 模型 ID（如 `gpt-4o`） |
| `name` | string | — | 显示名称 |
| `description` | string | — | 描述 |
| `envKey` | string | ✅ | 环境变量名（存 API Key） |
| `baseUrl` | string | — | API 端点 URL |
| `capabilities` | object | — | `{ vision: true }` 等 |
| `generationConfig` | object | — | 同 3.2，原子包，不从顶层继承 |

**唯一标识：** 同 authType 内，`id` + `baseUrl` 组合唯一

### 3.4 env — 环境变量

键值对，值可用 `$VAR` 引用其他环境变量。

```json
{
  "env": {
    "OPENAI_API_KEY": "sk-xxx",
    "ANTHROPIC_API_KEY": "sk-ant-xxx",
    "DEEPSEEK_API_KEY": "sk-xxx"
  }
}
```

### 3.5 mcpServers — MCP 服务器配置（核心）

```json
{
  "mcpServers": {
    "<SERVER_NAME>": {
      "command": "node",              // stdio 模式
      "args": ["server.js"],
      "env": { "KEY": "value" },
      "cwd": "/path/to/server",
      "url": "http://host/sse",       // SSE 模式
      "httpUrl": "http://host/mcp",   // Streamable HTTP 模式
      "headers": {},                  // HTTP 头
      "timeout": 30000,               // 超时 ms
      "trust": false,                 // 跳过确认
      "description": "描述",
      "includeTools": ["tool1"],      // 工具白名单
      "excludeTools": ["tool2"]       // 工具黑名单（优先于 include）
    }
  }
}
```

**连接优先级：** `httpUrl` > `url` > `command`，至少提供一个。

### 3.6 hooks — 事件钩子（核心）

三个事件点：

| 事件 | 触发时机 |
|---|---|
| `SessionStart` | 会话开始 |
| `SessionEnd` | 会话结束 |
| `PreToolUse` | 工具调用前 |

```json
{
  "hooks": {
    "SessionStart": [
      { "command": "bash ~/.qwen/hooks/start.sh" }
    ],
    "PreToolUse": [
      { "command": "bash ~/.qwen/hooks/pre-tool.sh" }
    ]
  }
}
```

### 3.7 permissions — 权限控制（核心）

**优先级：** `deny` > `ask` > `allow` > 默认

| 字段 | 类型 | 说明 |
|---|---|---|
| `permissions.allow` | string[] | 自动批准的工具规则 |
| `permissions.ask` | string[] | 需确认的工具规则 |
| `permissions.deny` | string[] | 禁止的工具规则 |

**规则语法：**
- `"Bash"` — 所有 shell 命令
- `"Bash(git *)"` — 以 git 开头的命令
- `"Read"` — 所有读操作（read_file, grep, glob, list_directory）
- `"Edit"` — 所有编辑操作（edit, write_file, notebook_edit）
- `"Read(/src/**/*.ts)"` — 路径匹配
- `"mcp__serverName"` — MCP 服务器所有工具

**路径前缀：** `//` 绝对路径、`~/` home 相对、`/` 项目根相对、`./` 工作目录相对

### 3.8 tools — 工具设置

| 字段 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `tools.approvalMode` | string | `"default"` | `plan`/`default`/`auto-edit`/`yolo` |
| `tools.sandbox` | boolean/string | `undefined` | 沙箱模式 |
| `tools.sandboxImage` | string | `undefined` | 沙箱镜像 |
| `tools.shell.enableInteractiveShell` | boolean | `false` | 交互式 shell |
| `tools.useRipgrep` | boolean | `true` | 使用 ripgrep |
| `tools.useBuiltinRipgrep` | boolean | `true` | 使用内置 ripgrep |
| `tools.truncateToolOutputThreshold` | number | `25000` | 截断阈值（字符） |
| `tools.truncateToolOutputLines` | number | `1000` | 截断行数 |
| `tools.discoveryCommand` | string | `undefined` | 自定义工具发现命令 |
| `tools.callCommand` | string | `undefined` | 自定义工具调用命令 |

### 3.9 context — 上下文配置

| 字段 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `context.fileName` | string/array | `undefined` | 上下文文件名 |
| `context.importFormat` | string | `undefined` | 导入格式 |
| `context.includeDirectories` | array | `[]` | 额外包含目录 |
| `context.loadFromIncludeDirectories` | boolean | `false` | 从包含目录加载 QWEN.md |
| `context.fileFiltering.respectGitIgnore` | boolean | `true` | 遵循 .gitignore |
| `context.fileFiltering.respectQwenIgnore` | boolean | `true` | 遵循 .qwenignore |
| `context.fileFiltering.enableRecursiveFileSearch` | boolean | `true` | 递归文件搜索 |
| `context.fileFiltering.enableFuzzySearch` | boolean | `true` | 模糊搜索 |
| `context.clearContextOnIdle.toolResultsThresholdMinutes` | number | `60` | 空闲清理阈值 |
| `context.clearContextOnIdle.toolResultsNumToKeep` | number | `5` | 保留最近工具结果数 |

### 3.10 memory — 记忆系统

| 字段 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `memory.enableManagedAutoMemory` | boolean | `true` | 自动记忆提取 |
| `memory.enableManagedAutoDream` | boolean | `true` | 自动记忆整理 |
| `memory.enableAutoSkill` | boolean | `true` | 自动技能审查 |

### 3.11 ui — 界面设置

| 字段 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `ui.theme` | string | `undefined` | 主题名 |
| `ui.customThemes` | object | `{}` | 自定义主题 |
| `ui.statusLine` | object | `undefined` | 状态栏配置 |
| `ui.hideWindowTitle` | boolean | `false` | 隐藏窗口标题 |
| `ui.hideTips` | boolean | `false` | 隐藏提示 |
| `ui.hideBanner` | boolean | `false` | 隐藏启动 Banner |
| `ui.hideFooter` | boolean | `false` | 隐藏页脚 |
| `ui.customBannerTitle` | string | `""` | 自定义 Banner 标题 |
| `ui.customBannerSubtitle` | string | `""` | 自定义 Banner 副标题 |
| `ui.customAsciiArt` | string/object | `undefined` | 自定义 ASCII 艺术 |
| `ui.showMemoryUsage` | boolean | `false` | 显示内存使用 |
| `ui.showLineNumbers` | boolean | `true` | 代码行号 |
| `ui.showCitations` | boolean | `true` | 显示引用 |
| `ui.renderMode` | string | `"render"` | Markdown 渲染模式 |
| `ui.compactMode` | boolean | `false` | 紧凑模式 |
| `ui.shellOutputMaxLines` | number | `5` | Shell 输出最大行数 |
| `ui.enableFollowupSuggestions` | boolean | `true` | 跟进建议 |
| `ui.enableCacheSharing` | boolean | `true` | 缓存共享 |
| `ui.enableSpeculation` | boolean | `false` | 推测执行 |
| `ui.accessibility.enableLoadingPhrases` | boolean | `true` | 加载短语 |
| `ui.accessibility.screenReader` | boolean | `false` | 屏幕阅读器模式 |
| `ui.customWittyPhrases` | array | `[]` | 自定义加载短语 |

### 3.12 security — 安全

| 字段 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `security.folderTrust.enabled` | boolean | `false` | 文件夹信任 |
| `security.auth.selectedType` | string | `undefined` | 当前认证类型 |
| `security.auth.enforcedType` | string | `undefined` | 强制认证类型 |
| `security.auth.useExternal` | boolean | `undefined` | 使用外部认证 |

### 3.13 slashCommands — 命令禁用

| 字段 | 类型 | 说明 |
|---|---|---|
| `slashCommands.disabled` | string[] | 禁用的斜杠命令名（大小写不敏感匹配） |

### 3.14 telemetry — 遥测

| 字段 | 类型 | 说明 |
|---|---|---|
| `telemetry.enabled` | boolean | 启用遥测 |
| `telemetry.target` | string | 目标标签（`local`/`gcp`） |
| `telemetry.otlpEndpoint` | string | OTLP 端点 |
| `telemetry.otlpProtocol` | string | 协议（`grpc`/`http`） |
| `telemetry.logPrompts` | boolean | 记录 prompt |
| `telemetry.includeSensitiveSpanAttributes` | boolean | 包含敏感属性 |
| `telemetry.outfile` | string | 输出文件路径 |

### 3.15 privacy — 隐私

| 字段 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `privacy.usageStatisticsEnabled` | boolean | `true` | 用量统计 |

### 3.16 ide — IDE 集成

| 字段 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `ide.enabled` | boolean | `false` | 启用 IDE 集成 |
| `ide.hasSeenNudge` | boolean | `false` | 已看过引导 |

### 3.17 advanced — 高级设置

| 字段 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `advanced.autoConfigureMemory` | boolean | `false` | 自动配置 Node.js 内存 |
| `advanced.dnsResolutionOrder` | string | `undefined` | DNS 解析顺序 |
| `advanced.excludedEnvVars` | array | `["DEBUG","DEBUG_MODE"]` | .env 排除变量 |
| `advanced.bugCommand.urlTemplate` | string | `undefined` | Bug 报告 URL 模板 |

### 3.18 codingPlan — 阿里云 Coding Plan

| 字段 | 类型 | 说明 |
|---|---|---|
| `codingPlan.region` | string | `china` / `global` |

### 3.19 experimental — 实验功能

| 字段 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `experimental.skills` | boolean | — | 技能系统 |
| `experimental.lsp` | boolean | — | LSP 支持 |
| `experimental.emitToolUseSummaries` | boolean | `true` | 工具调用摘要 |

### 3.20 output — 输出格式

| 字段 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `output.format` | string | `"text"` | `text` / `json` |

### 3.21 mcp — MCP 控制

| 字段 | 类型 | 说明 |
|---|---|---|
| `mcp.serverCommand` | string | MCP 服务器启动命令 |
| `mcp.allowed` | string[] | 允许的 MCP 服务器名 |
| `mcp.excluded` | string[] | 排除的 MCP 服务器名 |

---

## 4. 权限规则工具名别名

| 别名 | 实际工具 | 覆盖范围 |
|---|---|---|
| `Bash` / `Shell` | `run_shell_command` | — |
| `Read` / `ReadFile` | `read_file` + `grep_search` + `glob` + `list_directory` | 元类别 |
| `Edit` / `EditFile` | `edit` + `write_file` + `notebook_edit` | 元类别 |
| `Write` / `WriteFile` | `write_file` | — |
| `Grep` / `SearchFiles` | `grep_search` | — |
| `Glob` / `FindFiles` | `glob` | — |
| `ListFiles` | `list_directory` | — |
| `WebFetch` | `web_fetch` | — |
| `Agent` | `task` | — |
| `Skill` | `skill` | — |

---

## 5. Provider Model vs Runtime Model

| 维度 | Provider Model | Runtime Model |
|---|---|---|
| 配置来源 | `modelProviders` 中定义 | CLI/env/settings 层级解析 |
| 配置原子性 | 完整不可渗透包 | 分层逐字段解析 |
| 可复用性 | 始终在 `/model` 列表 | 作为快照捕获 |
| 团队共享 | 是（提交到 git） | 否（用户本地） |
| 凭据存储 | 仅通过 `envKey` 引用 | 快照中可能含实际 key |

---

## 6. 本地数据目录结构

```
~/.qwen/
├── settings.json              ← 用户级配置
├── QWEN.md                    ← 全局上下文/记忆
├── output-language.md         ← 输出语言偏好
├── installation_id            ← 安装 UUID
├── oauth_creds.json           ← OAuth 凭据
├── tip_history.json           ← 提示历史
│
├── projects/<project>/        ← 按项目组织（路径分隔符替换为 --）
│   ├── meta.json              ← 项目元数据
│   ├── chats/
│   │   ├── <session-id>.jsonl         ← 会话记录（JSONL）
│   │   └── <session-id>.runtime.json  ← 运行时元数据
│   ├── memory/                ← 项目级记忆
│   │   ├── MEMORY.md          ← 记忆索引
│   │   └── *.md               ← 具体记忆文件
│   └── subagents/             ← 子智能体会话
│       └── <session-id>/
│           ├── agent-*.jsonl
│           └── agent-*.meta.json
│
├── skills/                    ← 用户安装的技能
├── extensions/                ← 扩展
├── agents/                    ← Agent 定义文件
├── model/                     ← 模型配置文件
├── hooks/                     ← Hook 脚本
├── usage/                     ← 用量追踪
│   ├── usage.db               ← SQLite 数据库
│   └── config.yaml            ← 服务配置
├── plans/                     ← 计划文件
├── todos/                     ← 待办事项
├── file-history/              ← 文件历史
├── backup/                    ← 配置备份
├── debug/                     ← 调试日志
└── tmp/                       ← 临时目录
```

### 会话文件格式 (.jsonl)

每行一个 JSON 对象：

| 字段 | 类型 | 说明 |
|---|---|---|
| `uuid` | string | 消息唯一 ID |
| `parentUuid` | string | 父消息 ID（对话树） |
| `sessionId` | string | 会话 ID |
| `timestamp` | string | ISO 时间戳 |
| `type` | string | `user` / `assistant` / `system` / `tool_result` |
| `cwd` | string | 工作目录 |
| `version` | string | Qwen Code 版本 |
| `gitBranch` | string | Git 分支 |
| `message` | object | `{ role, parts }` 消息内容 |
| `subtype` | string | 系统消息子类型 |
| `systemPayload` | object | token 用量、工具调用信息 |
| `usageMetadata` | object | token 用量统计 |
| `agentId` | string | 子智能体 ID |
| `agentName` | string | 子智能体名称 |
| `isSidechain` | boolean | 是否为子智能体链 |

### 运行时文件格式 (.runtime.json)

| 字段 | 类型 | 说明 |
|---|---|---|
| `schema_version` | number | 格式版本 |
| `pid` | number | 进程 ID |
| `session_id` | string | 会话 ID |
| `work_dir` | string | 工作目录 |
| `hostname` | string | 主机名 |
| `started_at` | number | 开始时间戳 |
| `qwen_version` | string | Qwen Code 版本号 |
