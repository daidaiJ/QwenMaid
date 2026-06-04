# 待办迭代清单

## 实施优先级

| 阶段 | 内容 | 对应章节 |
|------|------|---------|
| **P0** | 小修复项 | #1 ~ #4 |
| **P1** | 基于 qwen-usage 的成本追踪 | #7 |
| **P2** | 技能市场 + 会话高级分析 | #5.4, #6 |
| **P3** | CLI 工具管理（qwen-usage + websearch 注册/自启/启停） | #5.1 ~ #5.3, #5.5 |

---

## 小修复项

### 1. 模型模态标签修正

**状态：** ✅ 无需处理（Qwen Code 内置根据 model ID 自动判断模态）

模型供应商配置的「输入模态」区域需要修正：

- **"PDF" 标签错误** — 当前 4 个选项是 `image/pdf/audio/video`，应该是 `image/text/audio/video`。"pdf" 应改为"纯文本"
- **默认选中问题** — 当前默认全部关闭（一个都没选中），应该默认选中"纯文本"。纯文本是所有模型的基础能力，不应该可关闭

**修改位置：** `src/components/proxy/ProviderPanel.tsx` → `ModelGenConfigEditor` 组件

---

### 2. 安装更新面板重设计

**状态：** 待重写

安装和更新检查面板（InstallPanel）需要重新设计。当前进入就直接开始检测，没有用户交互界面。

**目标设计：**

- **左栏：安装检测** — 检测 Node.js、npm、Qwen Code 是否已安装及版本
- **右栏：更新检测** — 检测 Qwen Code 当前版本 vs 最新版本
- **交互按钮流程：**
  1. 配置 Node.js 镜像源（国内加速，如 npmmirror）
  2. 安装 Qwen Code（`npm install -g @anthropic-ai/claude-code` 或对应包名）
  3. 更新 Qwen Code（`npm update -g`）

**修改位置：** `src/components/installer/InstallPanel.tsx`

---

### 3. 记忆/会话面板懒加载后中间区域无法加载

**状态：** ✅ 已修复（2026-06-04）

- GenericPanel `isGroup` 误判修复：`!item.level && (item.isGroup ?? ...)`
- `handleSelect` ID 前缀剥离：`id.split("::").slice(1).join("::")`
- 搜索模式隐藏展开箭头 + 箭头旋转动画
- GenericPanel 新增 `onItemClick` 拦截器和 `reloadGroupId` 支持
- SessionsPanel "加载更多"分页（每次 30 条）

---

### 3.5 总览页布局重排（待完成）

用户要求的新布局：
```
┌────────────────────┬─────────────────────────────┐
│  热力图（近30天）    │  ┌─────┐ ┌─────┐ ┌─────┐   │
│                    │  │总会话│ │总消息│ │总Tk │   │
│                    │  └─────┘ └─────┘ └─────┘   │
│                    │  ┌─────┐ ┌─────┐ ┌─────┐   │
│                    │  │缓存 │ │活跃天│ │Token│   │
│                    │  └─────┘ └─────┘ └─────┘   │
├────────────────────┼─────────────────────────────┤
│  项目统计 Top 5     │  模型趋势折线图（近30天）     │
├────────────────────┼─────────────────────────────┤
│  模型用量排名        │  工具调用排行                │
└────────────────────┴─────────────────────────────┘
```

**需要改的文件**：`src/components/analytics/AnalyticsPanel.tsx`

---

### 3.6 会话列表跳过空 Token 会话

**状态：** 待实现

会话列表中 `input_tokens === 0` 的会话应跳过不显示。

**改法**：前端 `loadGroupChildren` 中过滤，或后端 `list_sessions` 增加 token 信息过滤。

---

### 3.7 详情页性能指标平滑

**状态：** 待优化

当前 Catmull-Rom tension=0.08，用户仍觉得不够平滑。备选方案：
- 用半小时精度做插值（在相邻数据点之间插入中间点）
- 改用 `monotone` 插值（D3 风格，保证不过冲）

---

### 3.8 记忆/会话面板折叠展开 UX 改进

**状态：** 待重构

用户质疑：记忆文件和会话文件都是扁平文件，为什么用折叠展开？建议改为扁平列表 + 项目筛选下拉框。当前折叠展开功能可用但 UX 不理想。

---

### 4. 左栏会话信息展示优化

**状态：** 待优化

当前会话列表只显示一行（标题+消息数），缺少时间、模型、token 使用等关键信息，界面过于简陋。

**目标：**
- 参考 agentsview 的 sidebar index 设计，增加多行显示
- 显示最近会话时间、使用模型、消息条数、token 消耗等摘要

**参考：** `../agentsview/frontend/src/lib/api/types/core.ts` — `SidebarSessionIndexRow`

---

## 大需求

### 5. 外部工具管理面板（Unified Tool Manager）

**状态：** 待设计

AgentBox 作为统一管控面板，管理三类外部工具的安装、注册、启停。工具自身管理生命周期，AgentBox 只做配置注入和状态监控。

#### 5.1 统一管理模型

**部署目录：** `<AgentBox exe 所在目录>/bin/`

每个工具独立子目录，避免配置文件同名冲突：
```
AgentBox.exe
bin/
  qwen-usage/
    qwen-usage.exe         # 用量追踪 CLI
    config.json            # qwen-usage 自身配置
  websearch/
    websearch-mcpserver.exe # 搜索 MCP 服务
    config.yaml            # websearch 自身配置
    cache.db               # 搜索缓存（如有）
```

后端通过 `std::env::current_exe()` 拼接 `bin/<tool_id>/` 定位工具二进制和配置文件。

| 工具 | 类型 | 启动方式 | 注册方式 | AgentBox 管理项 |
|------|------|---------|---------|----------------|
| qwen-code-usage | 用量追踪 | VBS 静默启动 → Startup 目录快捷方式 | `ui.statusLine` + hooks → settings.json | 安装/更新/状态行注入/自启注册/启停 |
| websearch-mcpserver | MCP 服务 | VBS 静默启动 → Startup 目录快捷方式 | `mcpServers.websearch` (HTTP) → settings.json | 安装/注册/自启注册/启停/配置编辑 |
| 扩展（extensions） | Qwen Code 扩展 | 随 QC 启动 | `.qwen/extensions/` 目录 | 安装/启停 |

**自启动机制（统一）：** VBS 静默启动脚本 → Windows Startup 目录快捷方式

```
bin/<tool_id>/
  <tool>.exe
  start.vbs              # 隐藏窗口启动脚本（AgentBox 自动生成）
```

start.vbs 模板：
```vbs
Set WshShell = CreateObject("WScript.Shell")
WshShell.Run """<exe_path>"" server", 0, False
```

AgentBox 管理自启状态：
- **注册自启**：生成 start.vbs → 创建 Startup 目录快捷方式（`shell:startup`）
- **取消自启**：删除 Startup 目录快捷方式
- **检测自启**：检查 Startup 目录是否存在对应 .lnk 文件

**配置修改 → 重启流程：**

```
用户修改配置 (UI)
  │
  ├─→ 写入配置文件（config.yaml / config.json）
  │
  ├─→ 停止旧进程（tool stop / kill）
  │     └─ 等待进程退出（超时 3s → 强制 kill）
  │
  └─→ 启动新进程（tool start）
        └─ 健康检查（HTTP / version）确认就绪
```

**统一 Tauri 命令接口：**

```rust
// 工具发现与状态
#[tauri::command] fn list_managed_tools() -> Vec<ManagedTool>;
#[tauri::command] fn get_tool_status(tool_id: String) -> ToolStatus;

// 生命周期
#[tauri::command] fn install_tool(tool_id: String) -> Result<(), String>;
#[tauri::command] fn uninstall_tool(tool_id: String) -> Result<(), String>;
#[tauri::command] fn start_tool(tool_id: String) -> Result<(), String>;
#[tauri::command] fn stop_tool(tool_id: String) -> Result<(), String>;
#[tauri::command] fn restart_tool(tool_id: String) -> Result<(), String>;  // stop → wait → start → health check

// 配置
#[tauri::command] fn get_tool_config(tool_id: String) -> Result<Value, String>;
#[tauri::command] fn update_tool_config(tool_id: String, config: Value) -> Result<(), String>;
// update_tool_config 内部流程: 写文件 → restart_tool（自动）

// settings.json 注入
#[tauri::command] fn inject_tool_config(tool_id: String) -> Result<(), String>;
#[tauri::command] fn remove_tool_config(tool_id: String) -> Result<(), String>;
```

**数据结构：**

```typescript
interface ManagedTool {
  id: string;                    // "usage-tracker" | "websearch" | ...
  name: string;                  // 显示名称
  type: "usage" | "mcp" | "extension";
  description: string;
  installed: boolean;
  running: boolean;
  version?: string;
  latestVersion?: string;
  binaryPath: string;            // <exe_dir>/bin/<tool>.exe
  configPath?: string;           // 配置文件路径
  healthEndpoint?: string;       // 健康检查 URL
  settingsInjection: {
    path: string;                // settings.json 中的 JSON 路径
    value: unknown;              // 要注入的配置值
  };
}

interface ToolStatus {
  installed: boolean;
  running: boolean;
  version?: string;
  pid?: number;
  port?: number;
  uptime?: string;
  configValid: boolean;
  lastError?: string;
}
```

#### 5.2 用量追踪工具管理（qwen-code-usage）

**二进制路径：** `<exe_dir>/bin/qwen-usage/qwen-usage.exe`

**管理项：**
- 安装检测：`<bin>/qwen-usage/qwen-usage.exe version`
- 安装：下载预编译二进制到 `bin/qwen-usage/`
- 自启配置：确认 qwen-usage 自启动已就绪
- 状态行注入：一键写入 `ui.statusLine` + hooks 到 `~/.qwen/settings.json`
  ```json
  {
    "ui": { "statusLine": { "type": "command", "command": "input=$(cat); <bin>/qwen-usage/qwen-usage.exe record <<< \"$input\"" } },
    "hooks": {
      "SessionStart": [{ "hooks": [{ "type": "command", "command": "<bin>/qwen-usage/qwen-usage.exe start" }] }],
      "SessionEnd": [{ "hooks": [{ "type": "command", "command": "<bin>/qwen-usage/qwen-usage.exe stop" }] }]
    }
  }
  ```
  注入时动态替换 `<bin>` 为实际的 `bin/` 绝对路径
- 状态监控：读取 `~/.qwen/usage/usage.db` 确认数据在采集
- 数据展示：从 SQLite 读取用量数据 → AnalyticsPanel（见需求 #7）

#### 5.3 MCP 服务管理（websearch-mcpserver）

**二进制路径：** `<exe_dir>/bin/websearch/websearch-mcpserver.exe`

**管理项：**
- 安装检测：`<bin>/websearch/websearch-mcpserver.exe version` 或检查端口 health
- 安装：下载预编译二进制到 `bin/websearch/`
- 注册：写入 `mcpServers.websearch` 到 `~/.qwen/settings.json`
  ```json
  { "mcpServers": { "websearch": { "type": "http", "url": "http://localhost:8338/mcp" } } }
  ```
- 启停：`<bin>/websearch/websearch-mcpserver.exe start/stop`
- 配置编辑：读写 `bin/websearch/config.yaml`（端口、搜索模式、代理开关、工具开关）
- **配置修改后自动重启**：写入 config.yaml → stop → wait → start → health check
- 健康监控：`GET http://localhost:<port>/__admin/health`

**配置面板 UI：**
```
┌─ websearch-mcpserver ─────────────────────┐
│  状态: ● 运行中  v1.2.0  端口: 8338       │
│  [启动] [停止] [重启] [卸载]               │
├────────────────────────────────────────────┤
│  搜索模式:  ◉ engine (零Key)  ○ baidu     │
│             ○ tavily  ○ hybrid            │
│  代理:      ☐ 启用代理                     │
│  工具开关:  ☑ smartsearch  ☑ academic      │
│             ☐ cleanfetch  ☐ pdf_parser     │
│  [保存并重启服务]   ← 点击后写入配置+重启  │
└────────────────────────────────────────────┘
```

#### 5.4 技能市场（Skill Marketplace）

**数据来源：**

1. **skills.sh 公共目录** — `https://skills.sh/api/search?q=<query>&limit=<n>&offset=<n>`
2. **GitHub 仓库发现** — 用户添加仓库，后端扫描 SKILL.md
3. **本地 ZIP 安装**

**UI：** 搜索 + 卡片浏览 + 安装/卸载

**参考：** `../cc-switch/src/components/skills/`

#### 5.5 扩展管理

**管理项：**
- 扫描 `~/.qwen/extensions/` 已安装扩展
- 启停控制（重命名 `.disabled` 后缀）
- 如有公开 API 可加市场搜索，否则手动管理

**修改位置：** 新增 `src/components/tools/` 目录（ToolManagerPanel 统一入口）

---

### 6. 技能市场深度功能

**状态：** 待实现（依赖 5.4 基础）

详见需求 #5.4，市场搜索和安装为基础功能，后续可扩展：
- 评分/评论（如有 API）
- 依赖管理
- 版本锁定

---

### 7. 会话分析仪表盘（Session Analytics Dashboard）

**状态：** 部分实现（2026-06-04）

**已完成：**
- ✅ `AnalyticsPanel` Tab 切换（总览/详情）
- ✅ 详情页：模型多选 + Token 折线图 + 性能折线图 + 汇总卡片
- ✅ 后端 `metrics.rs`：usage.db 读取 + 动态列探测 + Go 时间格式适配
- ✅ 后端 `get_session_messages_paged` 分页命令（渐进式消息加载）
- ✅ usage.db Token 数据回填（prompt/completion/cached）
- ✅ TPS 计算（total_tokens * 1000 / latency_ms）
- ✅ 总览页双栏网格布局 + 项目统计 Top 5
- ✅ 热力图 compact 模式 + 周标签

**未完成：**
- 总览页布局重排（热力图移到 Row 1 左侧）— 见 #3.5
- 会话面板右栏分析面板（参考 agentsview SessionVitals）
- 消息工具调用分组（连续 tool call 合并）
- 虚拟滚动（大会话性能）
- 详情页性能曲线平滑优化 — 见 #3.7
- 会话列表跳过空 Token 会话 — 见 #3.6

参考 `../agentsview` 的会话分析功能，在 AgentBox 中实现全面的会话用量分析和可视化。

#### 6.1 后端：会话数据增强解析

当前 `readSession` 只返回基本消息文本，需增强解析：

**丰富消息结构（参考 agentsview）：**
```typescript
interface EnrichedMessage {
  uuid: string;
  type: string;              // user / assistant / system
  timestamp: string;
  model?: string;
  // 内容
  content: string;           // 主文本
  thinkingText?: string;     // 思考过程
  hasThinking: boolean;
  hasToolUse: boolean;
  // Token 使用
  tokenUsage?: {
    inputTokens: number;
    outputTokens: number;
    cacheCreationTokens: number;
    cacheReadTokens: number;
  };
  // 工具调用
  toolCalls?: ToolCall[];
}

interface ToolCall {
  toolName: string;
  category?: string;         // file / search / shell / agent / skill / ...
  toolUseId?: string;
  inputJson?: string;
  skillName?: string;        // 当 toolName=="Skill" 时记录技能名
  subagentSessionId?: string; // 当是子 agent 调用时
  resultContentLength?: number;
}
```

**后端 Rust 新增命令：**
- `get_session_stats` — 返回单会话统计（消息数、token、工具分布、模型列表、时长）
- `get_analytics_heatmap` — 返回 30 天使用热力图数据
- `get_analytics_summary` — 返回全局汇总（总会话数、总消息数、总 token、常用模型/工具）

**参考：** `../agentsview/internal/db/session_stats.go`、`../agentsview/internal/db/analytics.go`

#### 6.2 前端：会话统计面板（单会话）

在 SessionsPanel 右栏展示增强统计：

- **消息统计**：总数、用户/AI/系统 消息数
- **Token 用量**：输入/输出/缓存 token 分别统计
- **工具调用分布**：Top 10 工具 + 调用次数柱状图
- **技能调用**：列出使用过的 skill 名称和次数
- **子 Agent 调用**：列出 subagent 调用和关联 session ID
- **模型切换记录**：按时间线展示模型切换
- **会话时长**：首条→末条消息时间差

#### 6.3 前端：全局分析仪表盘（新面板）

新增顶级面板 "分析"（Analytics），包含：

**热力图：**
- 30 天使用热力图（GitHub-style 贡献图）
- 三种 metric 切换：消息数 / 会话数 / Token 消耗
- SVG 渲染，7行（周一至周日）× ~5列（4周）
- 5 级颜色强度（四分位算法）
- hover tooltip 显示日期和数值

**汇总卡片：**
- 总会话数、总消息数、总 Token 消耗、活跃天数
- 最常用模型、最常用工具
- 平均会话时长、平均消息数

**工具使用分析：**
- 工具调用按类别分布（file/search/shell/agent/skill/其他）
- Top 10 工具排行
- 技能调用列表 + 次数
- 子 Agent 调用统计

**模型使用分析：**
- 按模型的 Token 消耗占比
- 模型切换频率
- 各模型使用时长

**项目维度：**
- 按项目的会话数/消息数/Token 排行

**数据来源：** 扫描 `~/.qwen/projects/*/chats/*.jsonl`，后端聚合计算

**参考：**
- `../agentsview/frontend/src/lib/components/analytics/Heatmap.svelte`
- `../agentsview/frontend/src/lib/components/analytics/ToolUsage.svelte`
- `../agentsview/frontend/src/lib/components/usage/UsagePage.svelte`
- `../agentsview/internal/db/analytics.go` — `GetAnalyticsHeatmap`、`GetAnalyticsTools`
- `../agentsview/internal/db/session_stats_types.go` — `SessionStats` 完整结构

**修改位置：**
- 新增 `src/components/analytics/AnalyticsPanel.tsx`
- 新增 `src-tauri/src/commands/analytics.rs`（或扩展现有 filesystem.rs）
- `src/lib/tauri.ts` — 新增分析相关 API 函数
- `src/components/sessions/SessionsPanel.tsx` — 增强右栏统计

---
