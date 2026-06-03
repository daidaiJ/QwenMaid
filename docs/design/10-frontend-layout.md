# 10 — 前端布局与交互设计

> VS Code 风格布局，React 19 + shadcn/ui + Tailwind CSS 4 + Vite 7。

## 整体布局

```
┌──────────────────────────────────────────────────────┐
│                    标题栏                              │
├────┬─────────────────────────────────────────────────┤
│    │                                                 │
│ A  │              主内容区                            │
│ c  │  ┌──────────┬────────────────┬──────────────┐   │
│ t  │  │          │                │              │   │
│ i  │  │  左面板   │    中面板      │   右面板     │   │
│ v  │  │          │                │              │   │
│ i  │  │          │                │              │   │
│ t  │  │          │                │              │   │
│ y  │  │          │                │              │   │
│    │  │          │                │              │   │
│ B  │  │          │                │              │   │
│ a  │  │          │                │              │   │
│ r  │  └──────────┴────────────────┴──────────────┘   │
│    │                                                 │
├────┴─────────────────────────────────────────────────┤
│                    状态栏                              │
└──────────────────────────────────────────────────────┘
```

## Activity Bar（左侧功能导航栏）

宽度固定 48px，图标 + 文字标签，类似 VS Code：

| 图标 | 标签 | 对应页面 | 说明 |
|---|---|---|---|
| 🔧 | 配置 | Settings | settings.json 可视化编辑 |
| 🔌 | 代理 | Proxy | Provider/Model 管理 + 代理状态 |
| 📊 | 成本 | Costs | 成本追踪与统计图表 |
| 🧩 | 扩展 | Extensions | Skills + MCP + Hooks 管理 |
| 🔍 | 搜索 | Search | 内嵌搜索引擎 |
| 💾 | 记忆 | Memory | 项目记忆管理 |
| 📋 | 会话 | Sessions | 会话分析 |

点击图标切换主内容区页面，当前选中项高亮。

## 面板系统

### 弹性三栏布局

```
┌──────────┬────────────────┬──────────────┐
│  左面板   │    中面板      │   右面板     │
│  240px   │    flex-1      │   280px      │
│  (可调)  │    (主区域)     │  (可折叠)    │
└──────────┴────────────────┴──────────────┘
```

- **左面板**：列表/导航，固定宽度可拖拽调整
- **中面板**：主内容区，自适应填充
- **右面板**：辅助信息，可折叠/展开
- 面板数量根据页面动态调整（1-3 个）

### 面板交互

| 交互 | 说明 |
|---|---|
| 拖拽分割线 | 调整面板宽度 |
| 双击分割线 | 折叠/展开面板 |
| 右面板关闭按钮 | 折叠右面板，中面板扩展 |
| 快捷键 `Ctrl+B` | 切换左面板显示 |

## 页面与面板映射

| 页面 | 左面板 | 中面板 | 右面板 |
|---|---|---|---|
| **配置** | 配置分类树 | 字段表单编辑 | 字段说明 + 文档 |
| **代理** | Provider 列表 | Provider/Model 配置 | 连接状态 + 日志 |
| **成本** | 过滤器（日期/维度） | 热力图 + 折线图 + 数据表 | 单请求详情 + 同步状态 |
| **扩展** | 技能/MCP/Hooks 列表 | 内容编辑/预览 | 文件结构 + 统计 |
| **搜索** | 搜索类型 + 历史 | 搜索结果列表 | 结果详情预览 |
| **记忆** | 记忆文件列表 | Markdown 编辑器 | 元数据 + 操作 |
| **会话** | 项目/会话树 | 会话消息流 | 统计摘要 |

## 状态栏

底部固定高度，显示：
- 代理服务器状态（🟢 运行中 / 🔴 已停止）+ 端口号
- 当前 Qwen Code 版本
- 今日请求总数 / 总成本
- 上下文压缩状态（已启用/已禁用）

## 视觉参考

- **agentsView**（https://github.com/kenn-io/agentsview.git）：主内容区的视觉设计参考，卡片式布局、数据可视化、色彩搭配等作为样式样板
- **VS Code**：左侧 Activity Bar + 弹性面板的交互模式参考
- **cc-switch**：Provider 管理、代理状态等业务组件参考

## 技术实现

### 状态管理

使用 Zustand 轻量状态管理：

```typescript
// stores/proxy.ts
interface ProxyStore {
  providers: Provider[];
  models: Model[];
  isRunning: boolean;
  port: number;
  fetchProviders: () => Promise<void>;
  toggleProxy: () => Promise<void>;
}
```

### Tauri IPC 调用

```typescript
// lib/tauri.ts
import { invoke } from '@tauri-apps/api/core';

export async function getProviders(): Promise<Provider[]> {
  return invoke('list_providers');
}

export async function getCostSummary(filters: CostFilters): Promise<CostSummary> {
  return invoke('get_cost_summary', { filters });
}
```

### 路由

使用 React Router，Activity Bar 切换对应路由：

```tsx
<Routes>
  <Route path="/settings" element={<SettingsPage />} />
  <Route path="/proxy" element={<ProxyPage />} />
  <Route path="/costs" element={<CostsPage />} />
  <Route path="/extensions" element={<ExtensionsPage />} />
  <Route path="/search" element={<SearchPage />} />
  <Route path="/memory" element={<MemoryPage />} />
  <Route path="/sessions" element={<SessionsPage />} />
</Routes>
```

### 主题

- 默认暗色主题（匹配终端开发者习惯）
- 支持 shadcn/ui 内置主题切换
- 通过 CSS 变量实现主题色自定义
