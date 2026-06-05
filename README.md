# AgentBox

<p align="center">
  <img src="src-tauri/icons/128x128.png" width="96" alt="AgentBox" />
</p>

<p align="center">
  <a href="https://github.com/daidaiJ/AgentBox"><img alt="GitHub" src="https://img.shields.io/badge/GitHub-AgentBox-181717?logo=github&logoColor=white"></a>
  <a href="https://github.com/daidaiJ"><img alt="Author" src="https://img.shields.io/badge/Author-daidaiJ-orange"></a>
  <img alt="License" src="https://img.shields.io/badge/License-MIT-green">
  <img alt="Platform" src="https://img.shields.io/badge/Platform-Win%20%7C%20macOS%20%7C%20Linux-lightgrey">
</p>

**[Qwen Code](https://github.com/QwenLM/qwen-code) 的配套管理工具** — 供应商配置 / 代理转发 / 用量统计 / 会话分析 / MCP 网络服务

⭐ 觉得不错？[给个 Star](https://github.com/daidaiJ/AgentBox)

---

## 功能

### 可视化配置管理
图形化编辑 Qwen Code 的 `settings.json`，管理模型提供商、API Keys、环境变量。支持 13 个预设供应商模板一键导入。

### 本地路由代理
`localhost:18900` 本地代理服务器：
- **鉴权转换：** Bearer → x-api-key / x-goog-api-key / 自定义头
- **多供应商路由：** 按 billing_type、延迟、成功率加权选择
- **上下文压缩：** 集成 [only-cc-lite](https://github.com/daidaiJ/only-cc-lite)，Provider 级开关，请求转发前自动压缩（节省 40-90% tokens）
- **用量追踪：** 从 SSE 流实时提取 token 使用量、延迟、首字节时间

### 用量分析
- **总览 Tab：** 会话统计、模型排名、工具调用排行、热力图
- **详情 Tab：** 双数据源切换——"状态行 usage"（usage.db）/ "本地路由代理"（request_logs），Token 堆叠面积图 + 性能折线图（TPS/P50/P95）

### 会话分析
解析 Qwen Code 的 JSONL 会话文件，展示会话详情、消息内容、token 使用。

### MCP 网络服务
内嵌搜索引擎、学术检索、网页抓取，移植自 [websearch-mcpserver](https://github.com/daidaiJ/websearch-mcpserver)。支持 Bing / Baidu / Tavily 多引擎切换，百度千帆 API 接入。

### 状态行用量追踪
内嵌 [qwen-code-usage](https://github.com/daidaiJ/qwen-code-usage) CLI，实时采集 Qwen Code 状态行 Token 用量并写入 SQLite。

### 扩展管理
发现、安装、删除或禁用 Qwen Code 的技能、子智能体和 MCP 服务器。

### 记忆管理
可视化管理 Qwen Code 项目的 `.qwen/` 记忆文件。

## 系统托盘

- **关闭窗口：** 隐藏到系统托盘，不退出应用
- **托盘菜单：** 打开主界面 / MCP 服务状态切换 / 退出

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

## 技术栈

- **前端：** React 19 + TypeScript + Vite 8 + Tailwind CSS 4 + shadcn/ui
- **后端：** Rust (Tauri 2.x) + SQLite (rusqlite) + axum
- **压缩：** [only-cc-lite](https://github.com/daidaiJ/only-cc-lite)（零 ML 依赖的上下文压缩）

## 开发

```bash
npm install
npm run tauri dev    # 开发模式
npm run tauri build  # 构建
```

### 前置要求

- Node.js >= 18
- Rust toolchain（rustup）
- Tauri CLI (`cargo install tauri-cli`)

## 许可证

MIT
