# 09 — 记忆管理 (Memory Management)

> Qwen Code 项目记忆文件的浏览、编辑和管理。本质是文件夹内文本内容的展示。

## 数据来源

### 记忆文件结构

```
~/.qwen/
├── QWEN.md                          ← 全局用户记忆/指令
├── output-language.md               ← 输出语言偏好
│
├── projects/<project>/memory/       ← 项目级自动记忆
│   ├── MEMORY.md                    ← 记忆索引文件
│   ├── user_*.md                    ← 用户相关记忆
│   ├── feedback_*.md                ← 反馈记忆
│   ├── project_*.md                 ← 项目上下文记忆
│   └── reference_*.md               ← 外部引用记忆
```

### 记忆文件格式

每个记忆文件使用 frontmatter：

```markdown
---
name: user-role
description: 用户角色和职责
type: user
---

记忆内容正文...
```

MEMORY.md 索引文件：

```markdown
- [user-role](user_role.md) — 一行摘要
- [feedback-tool](feedback_tool.md) — 一行摘要
```

## 三栏布局

| 左栏 | 中栏 | 右栏 |
|---|---|---|
| 记忆文件列表 | 记忆内容编辑 | 元数据 + 操作 |

**左栏：**
- 全局记忆（~/.qwen/QWEN.md 等）
- 项目记忆分组（按项目目录）
- 每个文件显示：名称 + type 标签 + 摘要
- 类型筛选：user / feedback / project / reference

**中栏：**
- Markdown 编辑器（支持预览/源码切换）
- frontmatter 字段可视化编辑
- 语法高亮

**右栏：**
- 当前文件的 frontmatter 元数据
- 文件路径
- 最后修改时间
- 操作按钮：保存、删除、新建

## 操作

| 操作 | 说明 |
|---|---|
| 浏览 | 列出所有记忆文件，按类型分组 |
| 编辑 | 修改记忆内容和 frontmatter |
| 新建 | 创建新的记忆文件 |
| 删除 | 删除记忆文件并更新 MEMORY.md 索引 |
| 搜索 | 全文搜索记忆内容 |
| 刷新索引 | 重建 MEMORY.md 索引文件 |

## Tauri IPC 命令

| 命令 | 说明 |
|---|---|
| `list_memories(scope)` | 列出全局/项目级记忆 |
| `read_memory(path)` | 读取记忆文件 |
| `write_memory(path, content)` | 写入记忆文件 |
| `delete_memory(path)` | 删除记忆文件 |
| `search_memories(query)` | 全文搜索 |
| `rebuild_index(project)` | 重建 MEMORY.md 索引 |
