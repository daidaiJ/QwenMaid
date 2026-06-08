# 待办迭代清单

## 当前任务：前端 UI 风格优化 — Apple 扁平化设计语言

**状态：** 🔄 进行中（2026-06-06）

### 设计目标

参考 Apple Human Interface Guidelines 的扁平化设计语言，对 QwenMaid 全部前端页面进行视觉风格统一优化。核心原则：

1. **去线留白** — 减少硬边框（`border`），改用微妙的阴影和背景色差区分区域
2. **圆润感** — 增大 `border-radius`（卡片 12px、输入框 8px、按钮 6px）
3. **呼吸感** — 增大行高、段间距、组件内边距，字号从 9-13px 提升到 11-14px
4. **柔和色板** — 降低饱和度，背景用暖灰（`#f5f5f7`），强调色保持紫色系但降低使用频率
5. **状态栏瘦身** — 从紫色实底改为浅灰半透明底 + 彩色指示点
6. **侧边栏** — 更像 macOS Finder 侧边栏：无右侧硬边框，用阴影分隔

### 色板对照

| 语义 | 当前值 | 目标值 |
|------|--------|--------|
| `--bg-body` | `#f4f4f4` | `#f5f5f7`（Apple 经典暖灰） |
| `--bg-sidebar` | `#ececec` | `#f0f0f2`（更浅、更柔和） |
| `--bg-panel` | `#ffffff` | `#ffffff`（不变） |
| `--bg-card` | `#f8f8f8` | `#fafafa`（更轻盈） |
| `--bg-hover` | `#e8e8e8` | `#ececee`（暖灰 hover） |
| `--bg-selected` | `#e8e0f0` | `#eee8f5`（更柔和的选中态） |
| `--accent` | `#7c3aed` | `#7c3aed`（保持，但降低使用频率） |
| `--border` | `#e0e0e0` | `#e5e5ea`（Apple 标准分割线色） |
| `--shadow-card` | `0 1px 3px ...` | `0 0 0 1px rgba(0,0,0,0.04), 0 1px 3px rgba(0,0,0,0.06)` |
| `--shadow-dialog` | `0 8px 32px ...` | `0 16px 48px rgba(0,0,0,0.12), 0 2px 8px rgba(0,0,0,0.04)` |

### 涉及模块 & 文件

#### 1. 全局样式 — `src/index.css`
- [ ] 更新 `:root` CSS 变量色板（背景、边框、阴影）
- [ ] 增大默认字号基线（`body` 加 `font-size: 13px; line-height: 1.5`）
- [ ] 滚动条样式：更细（6px）、hover 时才显示 thumb
- [ ] 添加 `::selection` 选中色（紫色淡底）

#### 2. 布局外壳 — `src/components/layout/`
- **Shell.tsx** — [ ] 主面板圆角化（`main` 加 `rounded-tl-xl`），去掉 shadow，用背景色差分隔
- **ActivityBar.tsx** — [ ] 去掉 `border-r`，改用 `shadow-[1px_0_3px_rgba(0,0,0,0.04)]`；按钮 hover/active 用更圆润的 `rounded-lg`；图标尺寸微调
- **StatusBar.tsx** — [ ] 从紫色实底改为 `bg-[var(--bg-sidebar)]` + 左侧彩色圆点指示状态；文字改为 `text-[var(--text-secondary)]`；去掉白色文字
- **GenericPanel.tsx** — [ ] 左栏标题行增大高度和字号；列表项行高从 `h-8` 增到 `h-9`；搜索框圆角增大
- **ResizableColumns.tsx** — [ ] 分隔线从 1px 色块改为透明 + hover 时 2px 紫色细线

#### 3. 通用控件 — `src/components/config/FormControls.tsx`
- [ ] `inputCls`：`rounded-md` → `rounded-lg`，`h-9` → `h-[34px]`，去掉 `shadow-sm`
- [ ] `Toggle`：增大到 `h-[26px] w-12`，圆角用 `rounded-full`
- [ ] `TagInput` 标签：`rounded-sm` → `rounded-md`，增大内边距
- [ ] `Section` 标题：去掉 `uppercase`，改为正常大小写 + 更大字号（`text-[13px]`）

#### 4. 代理面板 — `src/components/proxy/`
- **ProviderPanel.tsx** — [ ] 预设卡片圆角增大、hover 阴影更柔和；弹窗背景加模糊效果
- **ProxyStatusPanel.tsx** — [ ] 状态卡片圆角增大（`rounded-lg` → `rounded-xl`）；统计数字字号增大；供应商头行间距增大

#### 5. 分析面板 — `src/components/analytics/`
- **AnalyticsPanel.tsx** — [ ] 汇总卡片圆角增大 + 阴影柔和化；SVG 图表区域背景更轻盈

#### 6. 其他面板（轻量调整）
- `SessionsPanel.tsx` — 会话列表行高增大
- `MemoryPanel.tsx` / `SkillsPanel.tsx` / `SubAgentsPanel.tsx` — 跟随 GenericPanel 统一调整
- `ConfigPanel.tsx` — 配置表单间距增大
- `AboutPanel.tsx` — 信息卡片圆角化

### 优先级

| 阶段 | 内容 | 涉及文件 |
|------|------|----------|
| P0 | 全局色板 + 布局外壳 | `index.css`, `Shell.tsx`, `ActivityBar.tsx`, `StatusBar.tsx` |
| P1 | 通用控件 | `FormControls.tsx`, `GenericPanel.tsx`, `ResizableColumns.tsx` |
| P2 | 各面板细节 | `ProviderPanel`, `ProxyStatusPanel`, `AnalyticsPanel` 等 |

### 组件复用性提升 — 防止代码膨胀

**原则：** 做 UI 优化时同步抽取可复用组件，避免各面板重复造轮子导致代码膨胀。

#### 待抽取的通用组件（`src/components/ui/`）

| 组件 | 当前散落位置 | 说明 |
|------|-------------|------|
| `StatCard` | `ProxyStatusPanel.tsx` 内联 | 汇总统计卡片（图标 + 标签 + 数值），`AnalyticsPanel` 也有类似模式 |
| `Card` | 各面板手写 `bg-[var(--bg-card)] rounded-lg border ...` | 统一卡片容器（圆角、阴影、边框），一处改全局生效 |
| `Dialog` | `ProviderPanel.tsx` 内联弹窗 | 统一遮罩 + 居中面板 + 标题栏 + 关闭按钮 |
| `Badge` | 各面板零散实现（`text-[9px] px-1 rounded ...`） | 统一标签/徽章样式（颜色变体：accent、success、error、muted） |
| `EmptyState` | 各面板手写「暂无数据」占位 | 统一空状态图标 + 主文案 + 副文案 |
| `ListItem` | `GenericPanel.tsx` 内部渲染 | 提取为独立组件，支持 hover/selected/active 状态 |

#### 实施策略

- **不单独开任务**，在 P0/P1/P2 各阶段改动时顺手抽取
- 优先抽取 `StatCard`、`Card`、`Dialog`（复用频率最高）
- 新组件放 `src/components/ui/` 目录，统一 export from `index.ts`
- 抽取后原面板改为 import，确保视觉效果不变

### 注意事项

- 所有颜色通过 `var(--xxx)` CSS 变量引用，不在组件中硬编码色值
- 布局类继续用 Tailwind，视觉样式用 CSS 变量
- 不引入新的 CSS 框架或组件库
- 保持现有功能不变，仅视觉层调整

---

## 历史待办（暂不处理，备忘）

| # | 内容 | 状态 |
|---|------|------|
| 2 | 安装更新面板重设计 | 待重写 |
| 3.5 | 总览页布局重排 | 待完成 |
| 3.6 | 会话列表跳过空 Token 会话 | 待实现 |
| 3.7 | 详情页性能指标平滑 | 待优化 |
| 3.8 | 记忆/会话面板折叠展开 UX 改进 | 待重构 |
| 4 | 左栏会话信息展示优化 | 待优化 |
| 5 | 外部工具管理面板 | 待设计 |
| 6 | 技能市场深度功能 | 待实现 |
