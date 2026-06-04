# InstallPanel 重设计 + 可拖拽布局 + 配置校验 + .env 智能写入

**日期:** 2026-06-04
**状态:** 已批准

---

## 1. 移除模态/上下文长度手动设置

Qwen Code 会根据 model ID 自动配置上下文长度和输入模态，无需在 AgentBox 中手动设置。

**修改位置:** `src/components/proxy/ProviderPanel.tsx` → `ModelGenConfigEditor`

**变更:**
- 移除「输入模态」区域（`modalities` 相关 UI 和状态）
- 移除「Context Window Size」数字输入框（保留字段透传，但不在 UI 显示）
- `GenerationConfig` 接口保留 `contextWindowSize` 和 `modalities` 字段（DB 存储兼容），但前端不暴露编辑

**Context Window 预设数据（仅参考，不展示给用户）：**

| 模型 | contextWindowSize |
|------|------------------|
| kimi-k2.6 / qwen3-coder-next / qwen3-coder-plus / qwen3.6-max-preview | 262144 |
| minimax-m2.5 / m2.7 | 204800 |
| kimi-k2.5 | 262144 |
| kimi-k2 | 131072 |
| glm-4 系列（除 4.6） | 131072 |
| glm-4.6 | 204800 |
| deepseek-v4 系列 | 1048576 |

---

## 3. InstallPanel 重设计

**参考:** cc-switch `AboutSection.tsx` 的工具卡片网格设计

**修改位置:** `src/components/installer/InstallPanel.tsx`（重写）

### 组件拆分

```
InstallPanel.tsx
├── EnvironmentCard.tsx     — 左栏：Node.js/npm/Qwen Code 安装状态
├── UpdateCard.tsx          — 右栏：版本对比 + 更新操作
├── ActionToolbar.tsx       — 底部：镜像源/安装/刷新按钮
└── MirrorConfigDialog.tsx  — 弹窗：npm 镜像源配置
```

### 布局

```
┌─────────────────────────────────────────────┐
│  安装与更新                                    │
│                                              │
│  ┌─────────────────┬─────────────────────┐  │
│  │ 本地环境          │ Qwen Code 更新       │  │
│  │                  │                     │  │
│  │ ✅ Node.js v20.x │ 当前: 1.0.50        │  │
│  │ ✅ npm 10.x      │ 最新: 1.0.52        │  │
│  │ ✅ Qwen Code     │ 🟡 有新版本          │  │
│  │   1.0.50         │                     │  │
│  │                  │ [立即更新]            │  │
│  ├─────────────────┴─────────────────────┤  │
│  │ 操作                                   │  │
│  │ [配置镜像源] [安装 Qwen Code] [刷新检测] │  │
│  └──────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

### 新增 Tauri 后端命令

| 命令 | 参数 | 返回 | 作用 |
|------|------|------|------|
| `detect_node_version` | — | `{ path, version } \| null` | 检测 Node.js 安装状态 |
| `detect_npm_version` | — | `{ path, version } \| null` | 检测 npm 安装状态 |
| `install_qwen_code` | `mirror?: string` | `Stream<string>` | 执行 `npm i -g @anthropic-ai/claude-code`，流式日志 |
| `update_qwen_code` | `mirror?: string` | `Stream<string>` | 执行 `npm update -g`，流式日志 |
| `configure_npm_mirror` | `registry: string` | `void` | `npm config set registry <url>` |

### 镜像源预设

- npm 官方: `https://registry.npmjs.org`
- npmmirror: `https://registry.npmmirror.com`
- 自定义输入

### 状态流

```
mount → 并行检测 [node, npm, qwen-code 本地版本, 最新版本]
       ↓
  EnvironmentCard 显示三项状态（✅/❌ + 版本号）
  UpdateCard 显示当前 vs 最新 + 状态
       ↓
  [安装] → install_qwen_code → 刷新状态
  [更新] → update_qwen_code → 刷新状态
  [配置镜像] → MirrorConfigDialog → configure_npm_mirror
  [刷新] → 重新检测全部
```

---

## 4. 三栏可拖拽布局

**依赖:** `react-resizable-panels`

**修改位置:** `src/components/layout/Shell.tsx`

### 布局结构

```
[ActivityBar 48px] [左栏 resizable] [中栏 flex-1] [右栏 resizable/collapsible] [StatusBar]
```

### PanelGroup 集成

```tsx
<PanelGroup direction="horizontal">
  <Panel defaultSize={20} minSize={15} maxSize={35}>
    {/* 左栏 */}
  </Panel>
  <PanelResizeHandle className="w-1 hover:bg-accent cursor-col-resize" />
  <Panel>
    {/* 中栏 */}
  </Panel>
  {hasRightPanel && <>
    <PanelResizeHandle className="w-1 hover:bg-accent cursor-col-resize" />
    <Panel defaultSize={25} minSize={15} maxSize={40} collapsible>
      {/* 右栏 */}
    </Panel>
  </>}
</PanelGroup>
```

### 布局配置接口

```ts
interface PanelLayout {
  left?: { content: ReactNode; defaultSize?: number; minSize?: number }
  center: { content: ReactNode }
  right?: { content: ReactNode; defaultSize?: number; minSize?: number; collapsible?: boolean }
}
```

### 各面板栏布局

| 面板 | 左栏 | 中栏 | 右栏 |
|------|------|------|------|
| ConfigPanel | 分类树 | 表单 | — |
| ProviderPanel | 供应商列表 | 供应商详情 + 模型列表 | 模型配置编辑器 |
| InstallPanel | 环境检测 | — | 更新检测 |
| 其他 Placeholder | — | 单栏 | — |

---

## 5. 同步前校验

**修改位置:** `src-tauri/src/config/mod.rs` → `sync_providers_to_settings`

### 校验规则

| 校验项 | 条件 | 级别 |
|--------|------|------|
| `api_key_env` 非空 | provider 的 envKey 为空 | error |
| `model_id` 非空 | model 的 model_id 为空 | error |
| `base_url` 非空 | provider 的 base_url 为空 | error |
| `env` 值存在 | settings.json 的 `env[envKey]` 未配置 | warning |

### 返回结构

```ts
interface SyncResult {
  valid: boolean
  errors: { field: string; providerId?: number; modelId?: number; message: string }[]
  warnings: { field: string; envKey: string; message: string }[]
  settings?: Value
}
```

### 前端交互

- valid=true 且无 warnings → 直接写入
- 有 warnings → 弹确认框，列出缺失的 envKey
- 有 errors → 阻断，显示错误列表

---

## 6. API Key 加密存储 + 环境变量存储模式

### 全局设置：envStorageMode

用户可选择 API Key 环境变量的存储位置，对所有供应商统一生效：

| 模式 | 值 | 行为 |
|------|-----|------|
| **合并模式**（默认） | `"merged"` | API Key 写入 settings.json 的 `env` 字段 |
| **分离模式** | `"separated"` | API Key 写入 `~/.qwen/.env`，settings.json 不含 `env` 字段 |

**存储位置:** AgentBox 本地配置（DB 或独立配置文件），不影响 Qwen Code settings.json

**前端 UI:** ConfigPanel 或 ProviderPanel 顶部的全局开关/选择器，带说明文字：
- 合并模式："API Key 写入 settings.json，方便但密钥与配置混在一起"
- 分离模式："API Key 写入 .env 文件，更安全，适合版本管理 settings.json 的场景"

### DB 扩展

```sql
ALTER TABLE providers ADD COLUMN api_key_value TEXT;  -- AES-256-GCM 加密
```

### 加密方案

- 算法: AES-256-GCM
- 密钥来源: `~/.qwen/.agentbox-key`（首次运行自动生成 32 字节随机密钥，文件权限 600）
- 每次加密生成随机 12 字节 nonce，密文格式: `nonce_base64:ciphertext_base64:tag_base64`

### 同步流程（根据模式分支）

```
sync_config_to_settings:
  1. 从 DB 读取 providers + models
  2. 校验（envKey/model_id/base_url 非空）
  3. 生成 modelProviders 条目
  4. 从 DB 解密 api_key_value → 生成 env 键值对

  if envStorageMode == "merged":
    5a. 写入 settings.json（modelProviders + env）
  else:
    5b. 写入 settings.json（仅 modelProviders，不含 env）
    5c. 写入 ~/.qwen/.env（KEY=VALUE 格式，备份旧文件）

  6. 返回 SyncResult
```

### 前端变更

Provider 创建/编辑表单新增：
- "API Key" 输入框（password 类型，脱敏显示）
- 保存时：加密 → 存入 DB 的 `api_key_value`
- 不再需要手动编辑 settings.json 的 `env` 字段

全局设置区域新增：
- `envStorageMode` 选择器（合并/分离）
- 切换时提示：切换后需重新同步配置

### 新增 Tauri 命令

| 命令 | 参数 | 返回 | 作用 |
|------|------|------|------|
| `write_env_file` | `keys: Map<String, String>` | `void` | 写入 `~/.qwen/.env`，备份旧文件 |
| `read_env_keys` | — | `Map<String, String>`（值脱敏） | 读取 `.env` 中的 key 列表 |
| `get_env_storage_mode` | — | `"merged" \| "separated"` | 读取当前模式 |
| `set_env_storage_mode` | `mode: string` | `void` | 设置模式 |

---

## 7. ActivityBar 扩展 + 新面板设计

### ActivityBar 更新

当前 7 个图标扩展为 9 个：

| 图标 | PanelId | 面板 | 说明 |
|------|---------|------|------|
| Settings | `config` | ConfigPanel | 配置管理（含 Hooks/MCP 子视图） |
| Server | `proxy` | ProviderPanel | 供应商/模型管理 |
| DollarSign | `cost` | CostPanel | 成本追踪（待实现） |
| Puzzle | `extensions` | ExtensionsPanel | 扩展市场 |
| Search | `search` | SearchPanel | 搜索（待实现） |
| Brain | `memory` | MemoryPanel | 记忆管理 |
| MessageSquare | `sessions` | SessionsPanel | 会话管理 |
| Bot | `subagents` | SubAgentsPanel | 子 Agent 管理 |
| Download | `install` | InstallPanel | 安装与更新 |
| Package | `skills` | SkillsPanel | 技能市场 |

---

### 8. Hooks / MCP 管理（ConfigPanel 子视图）

Hooks 和 MCP 是 settings.json 配置的一部分，作为 ConfigPanel 的分类子视图实现。

**修改位置:** `src/components/config/settingsSchema.ts` + `ConfigPanel.tsx`

**数据源:** settings.json 的 `hooks` 和 `mcpServers` 字段

**布局（复用 ConfigPanel 三栏）：**
- 左栏：配置分类树新增 "Hooks" 和 "MCP 服务器" 分类
- 中栏：
  - Hooks：按事件类型（PreToolUse/SessionStart/SessionEnd）分组显示
  - 每个 hook 条目：matcher + command，支持编辑和删除
  - MCP：服务器列表，每项显示 name + url/type，支持编辑和删除
- 右栏：选中项的 JSON 预览（只读审阅）

**交互：**
- 编辑：inline 表单编辑 matcher/command/url，保存前显示 diff 预览
- 删除：确认对话框
- 新增：底部 "+ 添加 Hook" / "+ 添加 MCP 服务器" 按钮

---

### 9. 技能市场（SkillsPanel）

**参考:** cc-switch `SkillsPage.tsx` + `UnifiedSkillsPanel.tsx`

**修改位置:** 新建 `src/components/skills/SkillsPanel.tsx`

**数据源:** `~/.qwen/skills/`（用户自定义）+ `~/.qwen/extensions/*/skills/`（扩展提供）

**布局（三栏）：**
- 左栏：技能列表（已安装 + 可发现），支持搜索过滤
- 中栏：选中技能的 SKILL.md 正文渲染（Markdown）
- 右栏：技能元数据（name/description/type from frontmatter）

**发现机制（简化版 cc-switch）：**
- 扫描本地 `~/.qwen/skills/` 和 `~/.qwen/extensions/*/skills/` 的 SKILL.md
- 解析 YAML frontmatter 获取 name/description
- 支持从 GitHub 仓库发现（可选，后续迭代）

**操作：**
- 查看：点击列表项，中栏渲染 SKILL.md
- 安装：从 GitHub ZIP 下载到 `~/.qwen/skills/`（参考 cc-switch 流程）
- 删除：确认后删除 `~/.qwen/skills/<name>/` 目录
- 扩展提供的技能标记为"扩展内置"，不可删除

**新增 Tauri 命令：**

| 命令 | 参数 | 返回 | 作用 |
|------|------|------|------|
| `list_skills` | — | `Skill[]` | 扫描本地技能列表 |
| `read_skill_content` | `path: string` | `string` | 读取 SKILL.md 内容 |
| `install_skill_from_github` | `owner, repo, branch?, dir?` | `Skill` | 从 GitHub 安装 |
| `delete_skill` | `directory: string` | `void` | 删除技能目录 |

---

### 10. 会话管理（SessionsPanel）

**参考:** agentsView（`/d/ai/agentsview`）的会话解析和统计逻辑

**数据源:** `~/.qwen/projects/<project>/chats/*.jsonl` + `*.runtime.json`

**布局（三栏）：**
- 左栏：项目列表 + 每个项目下的会话列表
  - 项目按 `meta.json` 的 lastDream 排序
  - 会话按 runtime.json 的 started_at 排序
  - 显示会话标题（从 JSONL 中 type=system, subtype=custom_title 提取）
  - 虚拟滚动（参考 agentsView SessionList）
- 中栏：选中会话的消息流渲染（虚拟滚动）
  - user 消息：用户气泡
  - assistant 消息：AI 回复（含工具调用折叠为 ToolGroup）
  - system 消息：灰色标注
  - thinking 块：可折叠
- 右栏：会话统计 + 时序分析（参考 agentsView SessionVitals）

**JSONL 解析（参考 agentsView `qwen.go`）：**

关键处理：
1. 连续 tool-call-only 的 assistant 条目合并为一个逻辑消息（`qwenAssistantBuffer` 模式）
2. 纯 tool-result 的 user 消息吸收到 assistant 缓冲区
3. Token 用量从 `usageMetadata` 提取：`promptTokenCount`、`candidatesTokenCount`、`cachedContentTokenCount`
4. 工具调用从 `functionCall` 提取，工具结果从 `functionResponse` 提取

**统计计算（简化版 agentsView `session_stats.go`）：**

| 指标 | 计算方式 |
|------|----------|
| 总消息数 | 所有消息计数 |
| 用户/AI/系统消息数 | 按 role 分组计数 |
| 总输出 Token | `usageMetadata.candidatesTokenCount` 累加 |
| 总输入 Token | `usageMetadata.promptTokenCount` 累加 |
| 缓存命中 Token | `usageMetadata.cachedContentTokenCount` 累加 |
| 工具调用次数 | `functionCall` 计数 |
| 工具调用分布 | 按工具名分组计数（含 Skill 调用次数统计） |
| 模型使用 | 从 assistant 消息的 `model` 字段提取，去重列表 |
| 会话时长 | 首条到最后一条消息的时间差 |
| Turn 周期 | 相邻 assistant 消息的时间间隔 P50/P90/均值 |
| 首响时间 | 第一条 user 到第一条 assistant 的时间 |
| Skill 调用统计 | 从 `functionCall.name` 筛选 skill 相关调用（如 `skill`/`activate_skill`），按 skill 名分组 |
| 子 Agent 调用 | 从 `functionCall.name` 筛选 agent 相关调用，按 agent 类型分组 |
| 30 天热力图 | 按天聚合会话耗时，类似 GitHub 贡献热力图，颜色深浅表示当天活跃度 |
| 模型使用分布 | 按模型统计使用次数和 token 用量，饼图或列表展示 |

**新增 Tauri 命令：**

| 命令 | 参数 | 返回 | 作用 |
|------|------|------|------|
| `list_projects` | — | `Project[]` | 扫描 `~/.qwen/projects/` |
| `list_sessions` | `project: string` | `Session[]` | 扫描项目下的会话 |
| `read_session` | `project, sessionId` | `Message[]` | 读取 JSONL 会话（合并 tool-call-only） |
| `get_session_stats` | `project, sessionId` | `SessionStats` | 计算会话统计 |

---

### 11. 记忆管理（MemoryPanel）

**数据源:** `~/.qwen/projects/<project>/memory/` + 全局 `~/.qwen/memory/`

**布局（三栏）：**
- 左栏：项目列表 + 记忆文件列表
  - 显示文件名和 frontmatter 的 type 标签（user/feedback/project/reference）
- 中栏：选中记忆文件的 Markdown 编辑器
  - 显示 YAML frontmatter（只读）+ 正文（可编辑）
  - 保存按钮
- 右栏：MEMORY.md 索引预览（只读）

**操作：**
- 查看：点击列表项，中栏渲染
- 编辑：直接在中栏编辑 Markdown，保存写回文件
- 新建："+ 新建记忆" 按钮，选择 type，输入 name
- 删除：确认后删除文件并更新 MEMORY.md 索引

**新增 Tauri 命令：**

| 命令 | 参数 | 返回 | 作用 |
|------|------|------|------|
| `list_memories` | `project?: string` | `MemoryFile[]` | 扫描记忆文件 |
| `read_memory` | `path: string` | `{ frontmatter, content }` | 读取记忆（解析 frontmatter） |
| `write_memory` | `path, content` | `void` | 写回记忆文件 |
| `delete_memory` | `path` | `void` | 删除并更新索引 |

---

### 12. 子 Agent 管理（SubAgentsPanel）

**数据源:** `~/.qwen/agents/*.md`（定义）+ `~/.qwen/projects/<project>/subagents/`（会话）

**布局（三栏）：**
- 左栏：Agent 定义文件列表（来自 `~/.qwen/agents/`）
  - 显示 agent 名称和类型
- 中栏：选中 Agent 的 Markdown 定义编辑
  - 显示 model、approvalMode、description 等 frontmatter
  - 支持编辑保存
- 右栏：该 Agent 的最近会话列表
  - 来自 `projects/<project>/subagents/`
  - 显示状态（running/completed）、创建时间、描述

**操作：**
- 查看/编辑 Agent 定义：中栏 Markdown 编辑器
- 查看 Agent 会话：点击右栏会话项，弹出消息流（复用 SessionsPanel 的渲染逻辑）
- 新建 Agent：模板 + Markdown 编辑
- 删除 Agent：确认后删除 .md 文件

**新增 Tauri 命令：**

| 命令 | 参数 | 返回 | 作用 |
|------|------|------|------|
| `list_agents` | — | `AgentDef[]` | 扫描 `~/.qwen/agents/` |
| `read_agent` | `name: string` | `{ frontmatter, content }` | 读取 Agent 定义 |
| `write_agent` | `name, content` | `void` | 写回 Agent 定义 |
| `delete_agent` | `name` | `void` | 删除 Agent |
| `list_agent_sessions` | `project, agentName?` | `AgentSession[]` | 扫描 Agent 会话 |

---

### 13. 扩展管理（ExtensionsPanel）

**数据源:** `~/.qwen/extensions/` + `extension-enablement.json`

**布局（三栏）：**
- 左栏：已安装扩展列表
  - 显示名称、版本（从 qwen-extension.json 读取）
  - 启用/禁用开关
- 中栏：选中扩展的详情
  - qwen-extension.json 元数据
  - 包含的 skills/hooks/commands/agents 列表
  - QWEN.md / GEMINI.md 上下文文件预览
- 右栏：扩展市场发现（可选，后续迭代）

**操作：**
- 启用/禁用：更新 `extension-enablement.json`
- 查看详情：中栏展示
- 删除：确认后删除扩展目录

**新增 Tauri 命令：**

| 命令 | 参数 | 返回 | 作用 |
|------|------|------|------|
| `list_extensions` | — | `Extension[]` | 扫描扩展列表 |
| `read_extension_detail` | `name` | `ExtensionDetail` | 读取扩展详情 |
| `toggle_extension` | `name, enabled` | `void` | 启用/禁用 |
| `delete_extension` | `name` | `void` | 删除扩展目录 |
