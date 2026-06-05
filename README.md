# AgentBox

统一管理 AI 编码助手的供应商配置、代理转发和用量统计的跨平台桌面应用。

## 技术栈

- **前端：** React 19 + TypeScript + Vite + Tailwind CSS 4 + shadcn/ui
- **后端：** Rust (Tauri 2.x) + SQLite (rusqlite) + axum
- **压缩：** [only-cc-lite](https://github.com/daidaiJ/only-cc-lite)（零 ML 依赖的上下文压缩）

## 功能

### 可视化配置管理
图形化编辑 Qwen Code 的 `settings.json`，管理模型提供商、API Keys、环境变量。支持 13 个预设供应商模板一键导入。

### 本地路由代理
`localhost:18900` 本地代理服务器，支持：
- **鉴权转换：** Bearer → x-api-key / x-goog-api-key / 自定义头
- **多供应商路由：** 按 billing_type、延迟、成功率加权选择
- **上下文压缩：** 集成 only-cc-lite，Provider 级开关，请求转发前自动压缩（节省 40-90% tokens）
- **用量追踪：** 从 SSE 流实时提取 token 使用量、延迟、首字节时间

### 用量分析
- **总览 Tab：** 会话统计、模型排名、工具调用排行、热力图
- **详情 Tab：** 双数据源切换——"状态行 usage"（usage.db）/ "本地路由代理"（request_logs），Token 堆叠面积图 + 性能折线图（TPS/P50/P95）

### 会话分析
解析 Qwen Code 的 JSONL 会话文件，展示会话详情、消息内容、token 使用。

### 技能/智能体/MCP 管理
发现、安装、删除或禁用 Qwen Code 的技能、子智能体和 MCP 服务器。

### 记忆管理
可视化管理 Qwen Code 项目的 `.qwen/` 记忆文件。

## 开发

```bash
# 安装依赖
npm install

# 开发模式（同时启动 Vite + Tauri）
npm run tauri dev

# 构建
npm run tauri build
```

### 前置要求

- Node.js >= 18
- Rust toolchain（rustup）
- Tauri CLI (`cargo install tauri-cli`)

## 架构

```
Qwen Code → POST localhost:18900/v1/messages
                ↓
         AgentBox 代理引擎 (axum)
           ├─ 路由解析（DB 查询）
           ├─ 鉴权转换
           ├─ 上下文压缩（only-cc-lite）
           ├─ 转发到上游 API
           ├─ UsageExtractor（SSE 流解析）
           └─ 写入 request_logs
                ↓
         上游 AI API (OpenAI / Anthropic / Gemini / ...)
```

## 许可证

MIT
