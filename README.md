# AgentBox

<p align="center">
  <img src="src-tauri/icons/128x128.png" width="96" alt="AgentBox" />
</p>

<p align="center">
  <a href="https://github.com/daidaiJ/QwenMaid"><img alt="GitHub" src="https://img.shields.io/badge/GitHub-AgentBox-181717?logo=github&logoColor=white"></a>
  <a href="https://github.com/daidaiJ"><img alt="Author" src="https://img.shields.io/badge/Author-daidaiJ-orange"></a>
  <img alt="License" src="https://img.shields.io/badge/License-MIT-green">
  <img alt="Platform" src="https://img.shields.io/badge/Platform-Win%20%7C%20macOS%20%7C%20Linux-lightgrey">
</p>

**[Qwen Code](https://github.com/QwenLM/qwen-code) 的桌面管理终端** — 供应商配置 · 代理路由 · 用量分析 · 会话浏览 · 子智能体编辑 · MCP 服务 · 扩展管理

⭐ 觉得不错？[给个 Star](https://github.com/daidaiJ/QwenMaid)

---

## 功能

### 供应商与模型管理

可视化管理 `settings.json` 中的模型提供商和 API Key。内置 13+ 预设供应商模板（OpenAI、Anthropic、DeepSeek、智谱 GLM、Kimi、MiMo、火山豆包、阶跃星辰、MiniMax、讯飞星火等），一键导入即可使用。

- **用户配置发现：** 自动解析 `settings.json` 中已有供应商配置，识别已配置的模型和 API Key
- **智能合并：** 新增供应商/模型时采用 merge 策略，保留用户已有配置不覆盖；仅上下文窗口大小和自定义鉴权头（如 `x-api-key`）强制同步预设值
- 按 `baseUrl` 域名 + `authType` 协议自动匹配预设，补齐缺失模型和 generationConfig

### 本地路由代理

内嵌 `axum` HTTP 代理引擎，监听 `localhost:18900`：

- **鉴权转换：** Bearer → x-api-key / x-goog-api-key / 自定义头，按供应商自动选择
- **多供应商路由：** 按计费类型、延迟、成功率加权选择最优上游
- **上下文压缩：** 集成 [only-cc-lite](https://github.com/daidaiJ/only-cc-lite)，节省 40-90% tokens
- **用量追踪：** 从 SSE 流实时提取 token 使用量和延迟指标

### 用量分析

双数据源切换——"状态行 usage"（usage.db）/ "本地路由代理"（request_logs）：

- **总览 Tab：** 会话统计、模型排名、工具调用排行、日历热力图
- **详情 Tab：** Token 堆叠面积图 + 性能折线图（TPS / P50 / P95 延迟）
- 图表 Tooltip 视口自适应：右侧空间不足时自动翻转到左侧显示
- 模型趋势折线图：按模型分组的 Token 消耗趋势，支持拖拽选区缩放

### 会话浏览器

解析 Qwen Code 的 JSONL 会话文件，分页加载，展示完整会话详情：

- 消息气泡：区分用户 / AI / 系统角色，显示模型名和 Token 统计
- Thinking 折叠块：可展开查看模型推理过程
- 工具调用：有预览内容时可折叠展开，无内容时简洁 inline 显示
- Markdown 渲染：消息内容支持完整 Markdown 语法

### 子智能体编辑器

管理 `.qwen/agents/` 下的智能体定义文件，三栏布局（列表 / 编辑器 / 元数据）：

- YAML frontmatter 交互式编辑：`model`、`approvalMode`、`color` 等字段支持下拉选择
- `model` 动态读取已配置模型列表，含 `inherit` / `fast` 快捷选项
- 保存时只替换 frontmatter，不破坏正文内容

### 配置编辑器

图形化编辑 Qwen Code 的 `settings.json`：

- Schema 驱动表单，支持嵌套路径读写
- API Key 脱敏显示，点击展开编辑
- 自动备份：每次写入前保存带时间戳的备份文件
- 环境变量和 `.env` 文件管理

### MCP 网络服务

内嵌 MCP 协议服务器，提供搜索 / 学术检索 / 网页抓取工具：

- 多引擎支持：Bing / Baidu / Tavily 切换
- 学术搜索：百度千帆学术 API 接入
- SSE 传输：标准 MCP SSE 协议，可被 Qwen Code 直接调用
- 调用统计：按工具维度记录成功/失败次数

### 技能管理

- 浏览、编辑、删除已安装的 Qwen Code 技能
- 显示来源（扩展 / 用户）和描述

### 扩展管理

- 列表展示已安装的 Qwen Code 扩展
- 启用 / 禁用切换
- 查看和编辑扩展上下文配置

### 记忆管理

可视化管理 Qwen Code 项目的 `.qwen/` 记忆文件，支持全局和项目级，Markdown 编辑器。

### Qwen Code 安装器

- 检测 / 安装 / 更新 Qwen Code（npm）
- npm 镜像源配置
- 版本比较和更新提示

### 状态行用量追踪

内嵌 [qwen-code-usage](https://github.com/daidaiJ/qwen-code-usage) CLI：

- 自动注入状态行配置到 settings.json
- 开机自启动支持
- 实时采集 Token 用量写入 SQLite

## UI 设计

- **Fluent Design 色彩系统：** oklch 色彩空间，亮色 / 暗色主题自动切换
- **可拖拽三栏布局：** 比例归一化 + 持久化
- **Toast 通知 + ErrorBoundary：** 全局错误捕获和用户提示
- **系统托盘：** 关闭窗口隐藏到托盘，不退出应用

## 架构

```
Qwen Code → POST localhost:18900/v1/messages
                ↓
         AgentBox 代理引擎 (axum)
           ├─ 路由解析（DB 查询，加权选择）
           ├─ 鉴权转换（自动匹配供应商头格式）
           ├─ 上下文压缩（only-cc-lite）
           ├─ 转发到上游 API
           ├─ UsageExtractor（SSE 流解析）
           └─ 写入 request_logs
                ↓
         上游 AI API (OpenAI / Anthropic / Gemini / DeepSeek / ...)
```

## 技术栈

- **前端：** React 19 + TypeScript + Vite 8 + Tailwind CSS 4
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

## 测试

```bash
# 单元测试（默认，不含集成测试）
cd src-tauri && cargo test --lib

# 含集成测试（启动 in-process mock 服务器，验证完整代理链路）
cd src-tauri && cargo test --lib --features integration
```

集成测试覆盖：鉴权转换、SSE 流 usage 提取、缓存 token 捕获、上下文压缩收益、request_logs 写入等。全部自包含，无需外部服务。

## 许可证

MIT
