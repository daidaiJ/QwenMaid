# 08 — 搜索集成 (Search Integration)

> 复用现有 websearch-mcpserver，AgentBox 提供搜索 UI 和 MCP 连接管理。

## 设计原则

AgentBox **不重新实现搜索引擎**，而是：
1. 连接已有的 `websearch-mcpserver`（运行在 `localhost:8338/mcp`）
2. 在前端提供搜索 UI，调用 MCP 工具获取结果
3. 搜索结果展示和管理

## MCP 服务器连接

websearch-mcpserver 使用 Streamable HTTP 协议：

```json
{
  "mcpServers": {
    "websearch": {
      "httpUrl": "http://localhost:8338/mcp",
      "description": "Web + 学术搜索引擎"
    }
  }
}
```

## 搜索 UI

三栏布局：

| 左栏 | 中栏 | 右栏 |
|---|---|---|
| 搜索类型选择 | 搜索结果列表 | 结果详情/网页预览 |

**左栏：**
- 通用搜索（smartsearch）
- 学术搜索（academicsearch）
- 网页抓取（cleanfetch）
- 搜索历史

**中栏：**
- 搜索输入框
- 结果列表（标题 + 摘要 + 来源）
- 分页

**右栏：**
- 选中结果的详细内容
- 网页预览（web_fetch）
- 引用格式导出

## Tauri IPC 命令

| 命令 | 说明 |
|---|---|
| `mcp_search(query, engine)` | 通过 MCP 调用搜索工具 |
| `mcp_fetch_url(url)` | 通过 MCP 抓取网页内容 |
| `get_search_history()` | 获取搜索历史 |
