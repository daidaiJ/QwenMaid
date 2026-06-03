# AgentBox — 整体架构概览

> Qwen Code 深度定制化工具箱，Tauri 桌面应用。

## 技术栈

| 层 | 技术 |
|---|---|
| 前端 | React 19 + shadcn/ui + Tailwind CSS 4 + Vite 7 |
| 后端 | Rust + Tauri 2 |
| 数据库 | SQLite + SeaORM (rusqlite 驱动) |
| 代理服务器 | axum (嵌入 Tauri 进程) |
| 上下文压缩 | headroom（本地算法，无网络调用） |

## 系统架构

```
┌─────────────────────────────────────────────────────────┐
│                    AgentBox (Tauri App)                  │
│                                                         │
│  ┌──────────┐   ┌─────────────────────────────────────┐ │
│  │ Activity │   │           主内容区                   │ │
│  │  Bar     │   │   左面板  │  中面板  │  右面板       │ │
│  │          │   │   (弹性 1-3 栏，按交互切换)          │ │
│  │ 🔧 配置  │   └─────────────────────────────────────┘ │
│  │ 🔌 代理  │                                           │
│  │ 📊 成本  │           React 19 Frontend               │
│  │ 🧩 扩展  │                                           │
│  │ 🔍 搜索  │   ┌─────────────────────────────────────┐ │
│  │ 💾 记忆  │   │        Tauri IPC Commands            │ │
│  └──────────┘   └──────────────┬──────────────────────┘ │
│                                │                         │
│  ┌─────────────────────────────┴───────────────────────┐ │
│  │                  Rust Backend                        │ │
│  │                                                     │ │
│  │  ┌────────────┐  ┌──────────────┐  ┌─────────────┐  │ │
│  │  │ Proxy      │  │ Config       │  │ Session     │  │ │
│  │  │ Engine     │  │ Manager      │  │ Analyzer    │  │ │
│  │  │ (axum)     │  │              │  │             │  │ │
│  │  │            │  │ - settings   │  │ - JSONL     │  │ │
│  │  │ - auth     │  │ - providers  │  │ - cost join │  │ │
│  │  │ - routing  │  │ - models     │  │ - stats     │  │ │
│  │  │ - compress │  │ - migration  │  │             │  │ │
│  │  │ - logging  │  │              │  │             │  │ │
│  │  └─────┬──────┘  └──────┬───────┘  └──────┬──────┘  │ │
│  │        │                │                 │         │ │
│  │  ┌─────┴────────────────┴─────────────────┴───────┐  │ │
│  │  │              SQLite (rusqlite)                  │  │ │
│  │  │  providers │ models │ request_logs │ settings   │  │ │
│  │  └────────────────────────────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
         ▲                            │
         │ localhost:18900            │ 转发
         │                            ▼
    ┌────┴─────┐            ┌──────────────────┐
    │  Qwen    │            │  Provider APIs   │
    │  Code    │            │  (OpenAI/Anthropic│
    │  CLI     │            │   Gemini/...)    │
    └──────────┘            └──────────────────┘
```

## 请求处理管道

```
Qwen Code 请求
  │
  ▼
┌─────────────────────┐
│ 1. 路由匹配          │  根据 endpoint + model 查表确定 provider/model
├─────────────────────┤
│ 2. 鉴权转换          │  Bearer → x-api-key (Anthropic) / 透传 (OpenAI)
├─────────────────────┤
│ 3. API 格式转换      │  chat/completions → responses (如需要)
├─────────────────────┤
│ 4. 上下文压缩(可选)  │  headroom 算法本地压缩，节省 token
├─────────────────────┤
│ 5. 转发到 Provider   │  使用 provider 的 baseUrl + auth_type 对应的 SDK 格式
├─────────────────────┤
│ 6. 流式响应透传      │  SSE 流直接转发回 Qwen Code
├─────────────────────┤
│ 7. 用量提取 & 记录   │  从响应中提取 usageMetadata，写入 SQLite
└─────────────────────┘
```

## 功能模块与优先级

| 阶段 | 模块 | 设计文档 | 说明 |
|---|---|---|---|
| **P0** | 代理引擎 | [01-proxy-engine.md](01-proxy-engine.md) | axum HTTP 代理核心 |
| **P0** | API 转换 | [02-api-transformation.md](02-api-transformation.md) | 鉴权映射 + 格式转换 |
| **P0** | 上下文压缩 | [03-context-compression.md](03-context-compression.md) | headroom 集成 |
| **P0** | 成本追踪 | [04-cost-tracking.md](04-cost-tracking.md) | SQLite schema + 聚合 |
| **P1** | 配置管理 | [05-config-management.md](05-config-management.md) | settings.json 可视化 |
| **P1** | 会话分析 | [06-session-analysis.md](06-session-analysis.md) | JSONL 解析 + 成本 join |
| **P1** | 技能/MCP/Hooks | [07-skill-mcp-hooks.md](07-skill-mcp-hooks.md) | 可视化管理 |
| **P2** | 搜索集成 | [08-search-integration.md](08-search-integration.md) | websearch-mcpserver |
| **P2** | 记忆管理 | [09-memory-management.md](09-memory-management.md) | 记忆文件浏览编辑 |
| — | 前端布局 | [10-frontend-layout.md](10-frontend-layout.md) | VS Code 风格布局 |

## 项目结构

```
AgentBox/
├── docs/
│   ├── design/                    ← 设计文档
│   ├── qwen-code-settings-reference.md
│   └── api/                       ← API 文档
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── proxy/                 ← 代理引擎
│   │   │   ├── mod.rs
│   │   │   ├── engine.rs          ← axum 服务器
│   │   │   ├── router.rs          ← 请求路由
│   │   │   ├── auth.rs            ← 鉴权转换
│   │   │   ├── transform/         ← API 格式转换
│   │   │   │   ├── mod.rs
│   │   │   │   ├── chat_to_responses.rs
│   │   │   │   └── responses_to_chat.rs
│   │   │   └── compress.rs        ← 上下文压缩
│   │   ├── db/                    ← 数据库层
│   │   │   ├── mod.rs
│   │   │   ├── schema.rs          ← 表定义
│   │   │   ├── provider.rs        ← Provider CRUD
│   │   │   ├── model.rs           ← Model CRUD
│   │   │   └── request_log.rs     ← 请求日志
│   │   ├── config/                ← 配置管理
│   │   │   ├── mod.rs
│   │   │   ├── reader.rs          ← settings.json 读取
│   │   │   ├── writer.rs          ← settings.json 写入
│   │   │   ├── schema.rs          ← 配置 schema 定义
│   │   │   └── migration.rs       ← 版本迁移
│   │   ├── session/               ← 会话分析
│   │   │   ├── mod.rs
│   │   │   ├── parser.rs          ← JSONL 解析
│   │   │   └── analyzer.rs        ← 统计分析
│   │   └── commands/              ← Tauri IPC 命令
│   │       ├── mod.rs
│   │       ├── proxy.rs
│   │       ├── config.rs
│   │       ├── session.rs
│   │       └── db.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                           ← React 前端
│   ├── App.tsx
│   ├── main.tsx
│   ├── components/
│   │   ├── layout/                ← VS Code 布局
│   │   │   ├── ActivityBar.tsx
│   │   │   ├── PanelLayout.tsx
│   │   │   └── StatusBar.tsx
│   │   ├── proxy/                 ← 代理管理页
│   │   ├── cost/                  ← 成本追踪页
│   │   ├── config/                ← 配置管理页
│   │   ├── session/               ← 会话分析页
│   │   ├── skills/                ← 技能管理页
│   │   ├── mcp/                   ← MCP 管理页
│   │   ├── search/                ← 搜索页
│   │   ├── memory/                ← 记忆管理页
│   │   └── ui/                    ← shadcn/ui 组件
│   ├── hooks/                     ← React hooks
│   ├── stores/                    ← 状态管理
│   └── lib/                       ← 工具函数
├── package.json
├── vite.config.ts
├── tailwind.config.ts
└── tsconfig.json
```
