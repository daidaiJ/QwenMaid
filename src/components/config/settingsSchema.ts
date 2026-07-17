/**
 * Qwen Code settings.json 可视化配置 Schema
 *
 * 基于 v0.16.2 settings-reference.md，定义前端表单渲染所需的元数据。
 * 每个字段声明类型、默认值、枚举选项、说明等，供 ConfigPanel 动态渲染。
 */

// ── 类型定义 ─────────────────────────────────────────────

export type FieldType =
  | "toggle"
  | "text"
  | "password"
  | "number"
  | "select"
  | "tags"
  | "path"
  | "json";

export interface FieldOption {
  value: string;
  label: string;
}

export interface SettingField {
  /** 配置路径，如 "general.vimMode" */
  path: string;
  /** 字段标签 */
  label: string;
  /** 字段类型，决定渲染控件 */
  type: FieldType;
  /** 说明文字 */
  description?: string;
  /** 默认值 */
  defaultValue?: unknown;
  /** select 的选项 */
  options?: FieldOption[];
  /** number 的最小值 */
  min?: number;
  /** number 的最大值 */
  max?: number;
  /** number 的步进 */
  step?: number;
  /** 单位文字 */
  unit?: string;
  /** tags 的建议列表 */
  suggestions?: string[];
  /** 密码类型占位符 */
  placeholder?: string;
  /** 是否需要重启生效 */
  requiresRestart?: boolean;
}

export interface SettingCategory {
  /** 分类 ID */
  id: string;
  /** 分类名称 */
  label: string;
  /** 图标名 (lucide) */
  icon: string;
  /** 分类说明 */
  description?: string;
  /** 该分类下的字段列表 */
  fields: SettingField[];
  /** 自定义渲染器标识（hooks / mcpServers 等复杂对象） */
  customRenderer?: "hooks" | "mcpServers";
}

// ── 辅助：从嵌套对象按路径取值 ───────────────────────────

export function getByPath(obj: Record<string, unknown>, path: string): unknown {
  const parts = path.split(".");
  let cur: unknown = obj;
  for (const p of parts) {
    if (cur == null || typeof cur !== "object") return undefined;
    cur = (cur as Record<string, unknown>)[p];
  }
  return cur;
}

// ── 辅助：按路径设置值（返回新对象） ─────────────────────

export function setByPath(
  obj: Record<string, unknown>,
  path: string,
  value: unknown
): Record<string, unknown> {
  const parts = path.split(".");
  const result = { ...obj };
  let cur: Record<string, unknown> = result;
  for (let i = 0; i < parts.length - 1; i++) {
    const p = parts[i]!;
    const next = cur[p];
    cur[p] =
      next != null && typeof next === "object"
        ? { ...(next as Record<string, unknown>) }
        : {};
    cur = cur[p] as Record<string, unknown>;
  }
  const last = parts[parts.length - 1]!;
  if (value === undefined) {
    delete cur[last];
  } else {
    cur[last] = value;
  }
  return result;
}

// ── 辅助：按路径删除值 ───────────────────────────────────

export function deleteByPath(
  obj: Record<string, unknown>,
  path: string
): Record<string, unknown> {
  return setByPath(obj, path, undefined);
}

// ── 配置分类定义（按关联程度聚合） ───────────────────────

export const settingCategories: SettingCategory[] = [
  // ═══════════════════════════════════════════════════════════
  // model — 模型与生成（合并原 model + generation）
  // ═══════════════════════════════════════════════════════════
  {
    id: "model",
    label: "模型与生成",
    icon: "Cpu",
    description: "模型选择、生成参数、会话限制",
    fields: [
      // ── 模型选择 ──
      {
        path: "model.name",
        label: "当前模型",
        type: "select",
        description: "默认使用的模型（从已配置的供应商模型中选择）",
        placeholder: "留空使用 Qwen Code 默认",
        options: [],
      },
      {
        path: "fastModel",
        label: "快速模型",
        type: "select",
        description: "轻量快速模型，用于摘要、补全等低开销任务",
        placeholder: "留空不启用",
        options: [],
      },
      // ── 生成参数 ──
      {
        path: "model.generationConfig.timeout",
        label: "请求超时",
        type: "number",
        description: "API 请求超时时间",
        defaultValue: 120000,
        min: 5000,
        max: 600000,
        step: 5000,
        unit: "ms",
      },
      {
        path: "model.generationConfig.maxRetries",
        label: "最大重试次数",
        type: "number",
        description: "请求失败后的最大重试次数",
        defaultValue: 3,
        min: 0,
        max: 10,
        step: 1,
        unit: "次",
      },
      {
        path: "model.generationConfig.samplingParams.temperature",
        label: "Temperature",
        type: "number",
        description: "控制输出随机性，0 = 确定性，1 = 高随机性",
        min: 0,
        max: 2,
        step: 0.1,
      },
      {
        path: "model.generationConfig.samplingParams.top_p",
        label: "Top-P",
        type: "number",
        description: "核采样阈值",
        min: 0,
        max: 1,
        step: 0.05,
      },
      {
        path: "model.generationConfig.samplingParams.max_tokens",
        label: "最大输出 Tokens",
        type: "number",
        description: "单次回复最大 token 数，留空自适应",
        min: 256,
        max: 1048576,
        step: 1024,
        unit: "tokens",
      },
      {
        path: "model.generationConfig.enableCacheControl",
        label: "缓存控制",
        type: "toggle",
        description: "启用 Anthropic prompt caching",
        defaultValue: false,
      },
      {
        path: "model.generationConfig.splitToolMedia",
        label: "拆分工具媒体内容",
        type: "toggle",
        description: "严格兼容服务器，将工具结果中的媒体内容拆分发送",
        defaultValue: false,
      },
      // ── 会话限制 ──
      {
        path: "model.maxSessionTurns",
        label: "最大会话轮次",
        type: "number",
        description: "单次会话最大交互轮次，-1 为无限制",
        defaultValue: -1,
        min: -1,
        step: 1,
        unit: "轮",
      },
      {
        path: "model.maxToolCalls",
        label: "工具调用预算",
        type: "number",
        description: "累计工具调用次数上限，-1 为无限制",
        defaultValue: -1,
        min: -1,
        step: 1,
        unit: "次",
      },
      {
        path: "model.maxWallTimeSeconds",
        label: "无头模式运行时间",
        type: "number",
        description: "无头模式下最大运行时间，-1 为无限制",
        defaultValue: -1,
        min: -1,
        step: 1,
        unit: "秒",
      },
      // ── 高级开关 ──
      {
        path: "model.skipNextSpeakerCheck",
        label: "跳过下一位发言者检查",
        type: "toggle",
        description: "禁用自动判断下一位发言者的逻辑",
        defaultValue: false,
      },
      {
        path: "model.skipLoopDetection",
        label: "跳过流式循环检测",
        type: "toggle",
        description: "禁用流式输出的重复循环检测",
        defaultValue: true,
      },
      {
        path: "model.skipStartupContext",
        label: "跳过启动上下文",
        type: "toggle",
        description: "启动时不自动加载工作区上下文",
        defaultValue: false,
      },
      {
        path: "model.enableOpenAILogging",
        label: "OpenAI API 日志",
        type: "toggle",
        description: "记录 OpenAI API 调用详情",
        defaultValue: false,
        requiresRestart: true,
      },
      // ── 压缩与截图 ──
      {
        path: "model.chatCompression.enableScreenshotTrigger",
        label: "截图触发压缩",
        type: "toggle",
        description: "工具返回的截图数量达到阈值时自动压缩上下文，防止电脑截图场景下上下文溢出",
        defaultValue: true,
      },
      {
        path: "model.chatCompression.screenshotTriggerThreshold",
        label: "截图压缩阈值",
        type: "number",
        description: "累积多少张工具截图后触发自动压缩",
        defaultValue: 50,
        min: 5,
        max: 500,
        step: 5,
        unit: "张",
      },
    ],
  },

  // ═══════════════════════════════════════════════════════════
  // general — 通用设置
  // ═══════════════════════════════════════════════════════════
  {
    id: "general",
    label: "通用",
    icon: "Settings",
    description: "编辑器偏好、更新、网络代理和会话行为",
    fields: [
      {
        path: "proxy",
        label: "网络代理",
        type: "text",
        description: "HTTP 代理地址，国内用户访问 API 时通常需要配置",
        placeholder: "http://127.0.0.1:7890",
      },
      {
        path: "general.preferredEditor",
        label: "首选编辑器",
        type: "select",
        description: "用于打开文件的编辑器",
        options: [
          { value: "", label: "默认" },
          { value: "vscode", label: "VS Code" },
          { value: "cursor", label: "Cursor" },
          { value: "windsurf", label: "Windsurf" },
          { value: "vim", label: "Vim" },
          { value: "nvim", label: "Neovim" },
          { value: "sublime", label: "Sublime Text" },
          { value: "webstorm", label: "WebStorm" },
        ],
      },
      {
        path: "general.vimMode",
        label: "Vim 键绑定",
        type: "toggle",
        description: "启用 Vim 风格的键绑定",
        defaultValue: false,
      },
      {
        path: "general.enableAutoUpdate",
        label: "自动更新",
        type: "toggle",
        description: "自动检查并安装更新",
        defaultValue: true,
      },
      {
        path: "general.terminalBell",
        label: "终端响铃",
        type: "toggle",
        description: "任务完成时终端发出响铃提示音",
        defaultValue: true,
      },
      {
        path: "general.showSessionRecap",
        label: "会话摘要",
        type: "toggle",
        description: "回归会话时显示上次离开时的摘要",
        defaultValue: false,
      },
      {
        path: "general.sessionRecapAwayThresholdMinutes",
        label: "摘要触发时间",
        type: "number",
        description: "离开多少分钟后触发会话摘要",
        defaultValue: 5,
        min: 1,
        max: 120,
        step: 1,
        unit: "分钟",
      },
      {
        path: "general.checkpointing.enabled",
        label: "会话检查点",
        type: "toggle",
        description: "启用会话检查点，支持回滚到之前的状态",
        defaultValue: false,
      },
      {
        path: "general.defaultFileEncoding",
        label: "默认文件编码",
        type: "select",
        description: "读写文件时使用的默认编码",
        defaultValue: "utf-8",
        options: [
          { value: "utf-8", label: "UTF-8" },
          { value: "utf-16", label: "UTF-16" },
          { value: "ascii", label: "ASCII" },
          { value: "latin1", label: "Latin-1" },
          { value: "gbk", label: "GBK" },
        ],
      },
      {
        path: "general.gitCoAuthor.commit",
        label: "Commit AI 署名",
        type: "toggle",
        description: "在 git commit 中添加 AI 署名行",
        defaultValue: true,
      },
      {
        path: "general.gitCoAuthor.pr",
        label: "PR AI 署名",
        type: "toggle",
        description: "在 Pull Request 中添加 AI 署名",
        defaultValue: true,
      },
    ],
  },

  // ═══════════════════════════════════════════════════════════
  // tools — 工具与审批
  // ═══════════════════════════════════════════════════════════
  {
    id: "tools",
    label: "工具与审批",
    icon: "Wrench",
    description: "审批模式、沙箱和工具行为",
    fields: [
      {
        path: "tools.approvalMode",
        label: "审批模式",
        type: "select",
        description:
          "plan = 先计划后执行 | default = 每次确认 | auto-edit = 自动编辑文件 | yolo = 全自动",
        defaultValue: "default",
        options: [
          { value: "plan", label: "Plan（先计划）" },
          { value: "default", label: "Default（每次确认）" },
          { value: "auto-edit", label: "Auto-Edit（自动编辑）" },
          { value: "yolo", label: "YOLO（全自动）" },
        ],
        requiresRestart: true,
      },
      {
        path: "tools.sandbox",
        label: "沙箱模式",
        type: "toggle",
        description: "在沙箱环境中执行 shell 命令",
        defaultValue: false,
      },
      {
        path: "tools.shell.enableInteractiveShell",
        label: "交互式 Shell",
        type: "toggle",
        description: "启用交互式 shell 模式",
        defaultValue: false,
      },
      {
        path: "tools.useRipgrep",
        label: "使用 ripgrep",
        type: "toggle",
        description: "搜索文件时使用 ripgrep（更快）",
        defaultValue: true,
      },
      {
        path: "tools.truncateToolOutputThreshold",
        label: "工具输出截断阈值",
        type: "number",
        description: "工具输出超过此字符数时截断",
        defaultValue: 25000,
        min: 1000,
        max: 200000,
        step: 1000,
        unit: "字符",
      },
      {
        path: "tools.truncateToolOutputLines",
        label: "工具输出截断行数",
        type: "number",
        description: "工具输出超过此行数时截断",
        defaultValue: 1000,
        min: 50,
        max: 10000,
        step: 50,
        unit: "行",
      },
    ],
  },

  // ═══════════════════════════════════════════════════════════
  // ui — 界面外观（合并原 ui + statusline + ide）
  // ═══════════════════════════════════════════════════════════
  {
    id: "ui",
    label: "界面外观",
    icon: "Palette",
    description: "主题、状态栏、显示选项、IDE 集成",
    fields: [
      // ── 显示控制 ──
      {
        path: "ui.hideWindowTitle",
        label: "隐藏窗口标题",
        type: "toggle",
        description: "隐藏终端窗口标题",
        defaultValue: false,
      },
      {
        path: "ui.hideTips",
        label: "隐藏提示",
        type: "toggle",
        description: "隐藏使用提示",
        defaultValue: false,
      },
      {
        path: "ui.hideBanner",
        label: "隐藏启动 Banner",
        type: "toggle",
        description: "隐藏启动时的 ASCII Art Banner",
        defaultValue: false,
      },
      {
        path: "ui.hideFooter",
        label: "隐藏页脚",
        type: "toggle",
        description: "隐藏底部页脚信息",
        defaultValue: false,
      },
      {
        path: "ui.showMemoryUsage",
        label: "显示内存使用",
        type: "toggle",
        description: "在界面中显示内存使用量",
        defaultValue: false,
      },
      {
        path: "ui.showLineNumbers",
        label: "显示行号",
        type: "toggle",
        description: "代码块显示行号",
        defaultValue: true,
      },
      {
        path: "ui.showCitations",
        label: "显示引用",
        type: "toggle",
        description: "显示来源引用信息",
        defaultValue: true,
      },
      {
        path: "ui.compactMode",
        label: "紧凑模式",
        type: "toggle",
        description: "减少界面间距，显示更多内容",
        defaultValue: false,
      },
      {
        path: "ui.shellOutputMaxLines",
        label: "Shell 输出最大行数",
        type: "number",
        description: "Shell 命令输出显示的最大行数",
        defaultValue: 5,
        min: 1,
        max: 100,
        step: 1,
        unit: "行",
      },
      {
        path: "ui.renderMode",
        label: "Markdown 渲染模式",
        type: "select",
        description: "Markdown 内容的渲染方式",
        defaultValue: "render",
        options: [
          { value: "render", label: "渲染模式" },
          { value: "raw", label: "原始文本" },
        ],
      },
      {
        path: "ui.enableFollowupSuggestions",
        label: "跟进建议",
        type: "toggle",
        description: "回复后显示跟进建议",
        defaultValue: true,
      },
      {
        path: "ui.enableCacheSharing",
        label: "缓存共享",
        type: "toggle",
        description: "允许多个会话共享缓存",
        defaultValue: true,
      },
      {
        path: "ui.enableSpeculation",
        label: "推测执行",
        type: "toggle",
        description: "提前推测可能的下一步操作",
        defaultValue: false,
      },
      {
        path: "ui.customBannerTitle",
        label: "自定义 Banner 标题",
        type: "text",
        description: "启动 Banner 的自定义标题",
        placeholder: "留空使用默认",
      },
      {
        path: "ui.customBannerSubtitle",
        label: "自定义 Banner 副标题",
        type: "text",
        description: "启动 Banner 的自定义副标题",
        placeholder: "留空使用默认",
      },
      {
        path: "ui.enableWelcomeBack",
        label: "欢迎回来提示",
        type: "toggle",
        description: "重新进入会话时显示欢迎提示",
        defaultValue: false,
      },
      {
        path: "ui.enableUserFeedback",
        label: "用户反馈",
        type: "toggle",
        description: "显示用户反馈入口",
        defaultValue: false,
      },
      // ── 状态栏（原 statusline 分类） ──
      {
        path: "ui.statusLine.type",
        label: "状态栏类型",
        type: "select",
        description: "状态栏内容来源类型",
        options: [
          { value: "", label: "默认" },
          { value: "command", label: "自定义命令" },
        ],
      },
      {
        path: "ui.statusLine.command",
        label: "状态栏命令",
        type: "path",
        description: "自定义状态栏内容的 shell 命令（需将类型设为 command）",
        placeholder: "bash ~/.qwen/statusline-command.sh",
      },
      // ── IDE 集成（原 ide 分类） ──
      {
        path: "ide.enabled",
        label: "启用 IDE 集成",
        type: "toggle",
        description: "启用与 IDE 的集成（如 VS Code 侧边栏）",
        defaultValue: false,
        requiresRestart: true,
      },
    ],
  },

  // ═══════════════════════════════════════════════════════════
  // context — 上下文配置
  // ═══════════════════════════════════════════════════════════
  {
    id: "context",
    label: "上下文",
    icon: "FileText",
    description: "文件过滤、上下文加载策略（默认值适用于大多数场景）",
    fields: [
      {
        path: "context.fileFiltering.respectGitIgnore",
        label: "遵循 .gitignore",
        type: "toggle",
        description: "文件搜索时遵循 .gitignore 规则",
        defaultValue: true,
      },
      {
        path: "context.fileFiltering.respectQwenIgnore",
        label: "遵循 .qwenignore",
        type: "toggle",
        description: "文件搜索时遵循 .qwenignore 规则",
        defaultValue: true,
      },
      {
        path: "context.fileFiltering.enableRecursiveFileSearch",
        label: "递归文件搜索",
        type: "toggle",
        description: "搜索文件时递归查找子目录",
        defaultValue: true,
      },
      {
        path: "context.fileFiltering.enableFuzzySearch",
        label: "模糊搜索",
        type: "toggle",
        description: "文件名搜索支持模糊匹配",
        defaultValue: true,
      },
      {
        path: "context.loadFromIncludeDirectories",
        label: "加载额外目录",
        type: "toggle",
        description: "从 includeDirectories 中加载 QWEN.md 上下文",
        defaultValue: false,
      },
      {
        path: "context.clearContextOnIdle.toolResultsThresholdMinutes",
        label: "空闲清理阈值",
        type: "number",
        description: "空闲多少分钟后清理工具结果",
        defaultValue: 60,
        min: 5,
        max: 480,
        step: 5,
        unit: "分钟",
      },
      {
        path: "context.clearContextOnIdle.toolResultsNumToKeep",
        label: "保留工具结果数",
        type: "number",
        description: "空闲清理时保留最近的工具结果数量",
        defaultValue: 5,
        min: 1,
        max: 50,
        step: 1,
        unit: "个",
      },
    ],
  },

  // ═══════════════════════════════════════════════════════════
  // memory — 记忆系统
  // ═══════════════════════════════════════════════════════════
  {
    id: "memory",
    label: "记忆系统",
    icon: "Brain",
    description: "自动记忆、梦境整理和技能审查",
    fields: [
      {
        path: "memory.enableManagedAutoMemory",
        label: "自动记忆提取",
        type: "toggle",
        description: "自动从对话中提取有价值的信息保存为记忆",
        defaultValue: true,
      },
      {
        path: "memory.enableManagedAutoDream",
        label: "自动记忆整理",
        type: "toggle",
        description: "定期整理和合并记忆文件",
        defaultValue: true,
      },
      {
        path: "memory.enableAutoSkill",
        label: "自动技能审查",
        type: "toggle",
        description: "启动时自动审查可用技能",
        defaultValue: true,
      },
    ],
  },

  // ═══════════════════════════════════════════════════════════
  // privacy — 隐私与安全（合并原 privacy + security + permissions）
  // ═══════════════════════════════════════════════════════════
  {
    id: "privacy",
    label: "隐私与安全",
    icon: "Shield",
    description: "遥测、数据收集、文件夹信任和工具权限规则",
    fields: [
      // ── 数据收集 ──
      {
        path: "privacy.usageStatisticsEnabled",
        label: "用量统计",
        type: "toggle",
        description: "允许收集匿名使用统计数据",
        defaultValue: true,
      },
      {
        path: "telemetry.enabled",
        label: "遥测",
        type: "toggle",
        description: "启用遥测数据上报",
        defaultValue: false,
      },
      {
        path: "telemetry.logPrompts",
        label: "记录 Prompt",
        type: "toggle",
        description: "遥测中包含完整的 prompt 内容（慎用）",
        defaultValue: false,
      },
      {
        path: "telemetry.otlpEndpoint",
        label: "OTLP 端点",
        type: "text",
        description: "OpenTelemetry 数据上报端点",
        placeholder: "http://localhost:4318",
      },
      {
        path: "telemetry.otlpProtocol",
        label: "OTLP 协议",
        type: "select",
        options: [
          { value: "grpc", label: "gRPC" },
          { value: "http", label: "HTTP" },
        ],
      },
      // ── 安全（原 security 分类） ──
      {
        path: "security.folderTrust.enabled",
        label: "文件夹信任",
        type: "toggle",
        description: "首次打开新文件夹时需要用户确认信任",
        defaultValue: false,
      },
      // ── 权限规则（原 permissions 分类） ──
      {
        path: "permissions.allow",
        label: "自动批准",
        type: "tags",
        description:
          "匹配的工具/命令自动批准，无需确认。如: Bash, Read, Edit, Bash(git *)",
        suggestions: [
          "Bash",
          "Read",
          "Edit",
          "Write",
          "Grep",
          "Glob",
          "WebFetch",
          "Bash(git *)",
          "Bash(npm *)",
          "Bash(pnpm *)",
          "Bash(cargo *)",
        ],
      },
      {
        path: "permissions.ask",
        label: "需要确认",
        type: "tags",
        description: "匹配的工具/命令需要用户确认",
        suggestions: [
          "Bash",
          "Read",
          "Edit",
          "Write",
          "Bash(rm *)",
          "Bash(git push*)",
        ],
      },
      {
        path: "permissions.deny",
        label: "禁止",
        type: "tags",
        description: "匹配的工具/命令被禁止执行",
        suggestions: [
          "Bash",
          "Edit",
          "Write",
          "Bash(rm -rf *)",
          "Bash(curl *)",
        ],
      },
    ],
  },

  // ═══════════════════════════════════════════════════════════
  // advanced — 高级设置
  // ═══════════════════════════════════════════════════════════
  {
    id: "advanced",
    label: "高级",
    icon: "Code",
    description: "Node.js 内存、DNS、实验功能",
    fields: [
      {
        path: "advanced.autoConfigureMemory",
        label: "自动配置 Node.js 内存",
        type: "toggle",
        description: "自动设置 Node.js --max-old-space-size",
        defaultValue: false,
        requiresRestart: true,
      },
      {
        path: "advanced.dnsResolutionOrder",
        label: "DNS 解析顺序",
        type: "select",
        options: [
          { value: "", label: "默认" },
          { value: "ipv4first", label: "IPv4 优先" },
          { value: "verbatim", label: "按返回顺序" },
        ],
      },
      {
        path: "experimental.emitToolUseSummaries",
        label: "工具调用摘要",
        type: "toggle",
        description: "生成工具调用的摘要信息",
        defaultValue: true,
      },
    ],
  },

  // ═══════════════════════════════════════════════════════════
  // mcpServers — MCP 服务器
  // ═══════════════════════════════════════════════════════════
  {
    id: "mcpServers",
    label: "MCP 服务器",
    icon: "Plug",
    description: "Model Context Protocol 服务器配置",
    fields: [],
    customRenderer: "mcpServers",
  },

  // ═══════════════════════════════════════════════════════════
  // hooks — 事件钩子
  // ═══════════════════════════════════════════════════════════
  {
    id: "hooks",
    label: "事件钩子",
    icon: "Webhook",
    description: "SessionStart / PreToolUse / SessionEnd 钩子脚本",
    fields: [],
    customRenderer: "hooks",
  },
];
