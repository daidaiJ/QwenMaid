# 05 — 配置管理 (Config Management)

> Qwen Code settings.json 的可视化编辑、版本感知和 schema 校验。

## 设计目标

- 图形化编辑 `~/.qwen/settings.json` 和 `<project>/.qwen/settings.json`
- 基于 qwen_version 做配置文件版本感知
- 编辑时做 schema 校验，防止格式错误
- 变更前自动备份
- 支持环境变量插值预览

## 配置文件定位

```
用户级:   ~/.qwen/settings.json          ← 主要管理目标
项目级:   <project>/.qwen/settings.json  ← 可选管理
系统级:   C:\ProgramData\qwen-code\settings.json  ← 只读展示
```

通过 Tauri 的 `fs` API 直接读写本地文件，无需代理层介入。

## 配置解析流程

```
读取 settings.json
  │
  ▼
JSONC 解析（支持注释）
  │
  ▼
版本检测（从 runtime.json 读取 qwen_version）
  │
  ▼
Schema 版本匹配
  │
  ▼
如有需要 → 执行迁移
  │
  ▼
环境变量插值解析（$VAR / ${VAR}）
  │
  ▼
返回结构化配置对象给前端
```

## Schema 定义

每个配置字段定义完整的元信息：

```rust
struct ConfigField {
    path: String,              // 如 "model.generationConfig.timeout"
    field_type: FieldType,     // string | number | boolean | object | array
    description: String,       // 说明
    default_value: Option<Value>,
    enum_values: Option<Vec<String>>,  // 可选值
    since_version: String,     // 引入版本
    deprecated: bool,
    deprecated_message: Option<String>,
    requires_restart: bool,
    category: String,          // 所属分类
}
```

配置 schema 以 Rust 代码硬编码（基于 v0.16.2 文档），版本更新时同步更新。

## 版本感知与迁移

### 版本检测

```rust
fn detect_qwen_version() -> Option<String> {
    // 1. 检查 ~/.qwen/projects/*/chats/*.runtime.json 中的 qwen_version
    // 2. 尝试执行 `qwen --version`
    // 3. 回退到 settings.json 中推断
}
```

### 迁移规则（v0.16.2 → 未来版本）

维护一份迁移规则表：

```rust
struct MigrationRule {
    from_version: String,
    to_version: String,
    field_path: String,        // 旧字段路径
    new_field_path: String,    // 新字段路径
    transform: TransformType,  // rename | move | remove | invert_bool
}
```

当前已知迁移规则（文档中记录），新版本发布时通过 diff 更新文档和代码。

## 前端交互

### 配置编辑界面

三栏布局：
- **左栏**：配置分类树（general / model / tools / permissions / ...）
- **中栏**：选中分类的字段列表，表单编辑
- **右栏**：当前字段的说明、默认值、关联文档链接

### 编辑操作

| 操作 | 说明 |
|---|---|
| 直接编辑 | 表单输入，实时 schema 校验 |
| 环境变量引用 | 输入框支持 `$VAR` 语法，显示解析后的值（脱敏） |
| JSON 编辑器 | 高级模式，直接编辑 JSON（带 schema 提示） |
| 差异预览 | 保存前显示变更 diff |
| 一键备份 | 保存前自动备份到 `~/.qwen/backup/settings.json.<timestamp>` |
| 重置为默认 | 单个字段重置为默认值 |

### 敏感字段处理

API Key 等敏感字段：
- 存储在 `env` 对象中，值通过环境变量引用
- 前端显示为 `••••••`，可点击"显示"按钮
- 编辑时使用密码输入框
- 不通过 Tauri IPC 传输完整 key 值（仅传输引用名）

## Tauri IPC 命令

| 命令 | 说明 |
|---|---|
| `read_settings(scope)` | 读取用户级/项目级 settings.json |
| `write_settings(scope, content)` | 写入（自动备份） |
| `get_config_schema(category)` | 获取配置 schema 定义 |
| `validate_settings(content)` | 校验配置内容 |
| `get_env_vars()` | 获取当前环境变量列表（脱敏） |
| `detect_qwen_version()` | 检测 Qwen Code 版本 |

## 配置快照与回滚

用户可对当前配置状态做快照，用于后续回滚。快照包含三部分：

### 快照内容

| 组成 | 来源 | 格式 |
|---|---|---|
| Qwen Code 配置 | `~/.qwen/settings.json` | JSON 原文 |
| 供应商配置 | AgentBox DB `providers` 表 | JSON 序列化 |
| 模型配置 | AgentBox DB `models` 表 | JSON 序列化 |

### 快照存储

```
~/.qwen/agentbox-snapshots/
├── 2026-06-03T14-30-00/
│   ├── manifest.json        ← 快照元数据（时间、描述、版本）
│   ├── settings.json        ← Qwen Code 配置副本
│   └── agentbox-config.json ← providers + models 快照
├── 2026-06-01T10-00-00/
│   └── ...
```

`manifest.json`：
```json
{
  "id": "2026-06-03T14-30-00",
  "created_at": "2026-06-03T14:30:00Z",
  "description": "用户手动快照",
  "qwen_version": "0.16.2",
  "settings_hash": "sha256:...",
  "providers_count": 3,
  "models_count": 8
}
```

### 快照操作

| 操作 | 说明 |
|---|---|
| 创建快照 | 用户主动触发，读取当前三部分配置打包存储 |
| 列出快照 | 按时间倒序展示，显示摘要信息 |
| 预览快照 | 展示快照内容与当前配置的 diff |
| 应用快照 | 将快照内容写回对应位置（应用前自动创建当前状态快照） |
| 删除快照 | 清理不需要的快照 |

### 快照与自动备份的区别

| | 自动备份 | 手动快照 |
|---|---|---|
| 触发时机 | 每次保存 settings.json 时自动创建 | 用户主动触发 |
| 内容范围 | 仅 settings.json | settings.json + providers + models |
| 存储位置 | `~/.qwen/backup/` | `~/.qwen/agentbox-snapshots/` |
| 用途 | 单文件回滚 | 全局配置状态回滚 |

### IPC 命令

| 命令 | 说明 |
|---|---|
| `create_snapshot(description)` | 创建快照（含 settings.json + DB providers/models） |
| `list_snapshots()` | 列出所有快照摘要 |
| `get_snapshot_diff(snapshot_id)` | 快照与当前配置的差异 |
| `apply_snapshot(snapshot_id)` | 应用快照（应用前自动备份当前状态） |
| `delete_snapshot(snapshot_id)` | 删除快照 |
