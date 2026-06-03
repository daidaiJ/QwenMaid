# 07 — 技能/MCP/Hooks 管理

> 可视化管理 Qwen Code 的技能、MCP 服务器、Hooks 和扩展。

## 数据来源

所有数据来自 `~/.qwen/` 目录下的文件系统，通过 Tauri `fs` API 直接读写。

### 技能 (Skills)

```
~/.qwen/skills/<skill-name>/SKILL.md       ← 用户安装的技能
~/.qwen/extensions/<ext-name>/SKILL.md     ← 扩展中的技能
```

内置技能在 npm 包目录 `bundled/` 下，只读。

### MCP 服务器

配置在 `settings.json` 的 `mcpServers` 对象中。

### Hooks

配置在 `settings.json` 的 `hooks` 对象中，脚本文件在 `~/.qwen/hooks/`。

### 扩展 (Extensions)

```
~/.qwen/extensions/<ext-name>/
├── SKILL.md
├── package.json
└── ...
```

启用配置在 `~/.qwen/extensions/extension-enablement.json`。

## 三栏布局设计

### 技能管理

| 左栏 | 中栏 | 右栏 |
|---|---|---|
| 技能列表（名称 + 类型标签） | SKILL.md 内容展示/编辑 | 文件结构树 + 调用统计 |

**左栏列表项信息：**
- 名称（来自 SKILL.md frontmatter 的 `name` 字段）
- 类型标签：`bundled` / `extension` / `skill`
- 启用/禁用状态开关
- 描述摘要（`description` 字段，截断）

**中栏内容：**
- SKILL.md 的 Markdown 渲染
- 支持编辑模式（切换到源码编辑）
- frontmatter 字段可视化编辑

**右栏辅助：**
- 技能目录文件树
- 从会话数据统计的调用次数（grep `skill` 相关记录）

### MCP 服务器管理

| 左栏 | 中栏 | 右栏 |
|---|---|---|
| 服务器列表 | 配置详情 + 连接状态 | 工具列表 |

**左栏列表项：**
- 服务器名称
- 连接状态指示（🟢/🔴/🟡）
- 连接类型标签（stdio / SSE / HTTP）

**中栏内容：**
- 配置 JSON 编辑
- 测试连接按钮
- 信任设置

**右栏工具列表：**
- 已发现的工具名列表
- 每个工具的描述
- includeTools / excludeTools 配置

### Hooks 管理

| 左栏 | 中栏 | 右栏 |
|---|---|---|
| 事件列表（SessionStart/End/PreToolUse） | Hook 脚本内容 | 执行日志 |

**左栏：**
- 三个事件分组
- 每个事件下的 hook 脚本列表
- 启用/禁用开关

**中栏：**
- 脚本内容编辑器
- 执行命令预览

**右栏：**
- 最近执行日志
- 执行耗时统计

## 操作

| 操作 | 说明 |
|---|---|
| 启用/禁用 | 切换技能/MCP/Hook 的启用状态 |
| 删除 | 删除技能/MCP/Hook 配置 |
| 新建 | 创建新的 MCP 服务器配置 / Hook |
| 编辑 | 修改配置内容 |
| 测试连接 | 测试 MCP 服务器连接 |
| 导入 | 从 URL 或本地路径导入技能 |

## Tauri IPC 命令

| 命令 | 说明 |
|---|---|
| `list_skills()` | 列出所有技能（bundled + extension + skill） |
| `read_skill(name, type)` | 读取技能内容 |
| `write_skill(name, content)` | 写入技能内容 |
| `toggle_skill(name, enabled)` | 启用/禁用 |
| `list_mcp_servers()` | 列出 MCP 服务器 |
| `save_mcp_server(name, config)` | 保存 MCP 配置 |
| `test_mcp_connection(name)` | 测试连接 |
| `list_hooks()` | 列出所有 Hooks |
| `save_hook(event, config)` | 保存 Hook 配置 |
| `toggle_hook(event, name, enabled)` | 启用/禁用 Hook |
