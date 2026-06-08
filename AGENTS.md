# AGENTS.md — AgentBox 开发指南

## 通过预设更新添加新模型 / 新供应商

### 数据流概览

```
provider-presets.json（编译时内嵌）
    ↓ load_presets()
    ↓ discover_existing_providers: 预设模型补齐 → config_json（含 contextWindowSize / maxOutputTokens）→ DB
    ↓ sync_preset_models_to_settings: DB → generationConfig → 用户 settings.json
```

预设文件是**数据源**，用户 settings.json 是**生成产物**。修改预设后，用户下次「发现供应商」即可同步新模型。

### 预设文件位置

| 文件 | 用途 |
|------|------|
| `src-tauri/resources/provider-presets.json` | 运行时内嵌预设（编译时 `include_str!` 打包进二进制） |
| `docs/qwenCodeProviderPresets.ts` | TS 参考文件（含完整字段定义，作为数据权威来源） |

### JSON 结构

```jsonc
[
  {
    "name": "供应商显示名称",
    "baseUrl": "API 端点（代理层用）",
    "envPrefix": "环境变量名，如 OPENAI_API_KEY",
    "proxyMode": "direct",
    "billingType": "pay_per_use | plan",
    "authHeader": "可选，非标准鉴权头，如 x-api-key / api-key",
    "models": [
      {
        "id": "model-id",           // 必须在该供应商内唯一
        "name": "model-id",         // 尽量与 id 相同
        "authType": ["openai"],     // 支持的协议：openai / anthropic / gemini
        "contextWindowSize": 131072, // 上下文窗口大小（tokens）
        "maxOutputTokens": 65536     // 最大输出 tokens（可选）
      }
    ]
  }
]
```

### 添加新模型（已有供应商）

1. 打开 `src-tauri/resources/provider-presets.json`
2. 在目标供应商的 `models` 数组中追加条目
3. 确保 `id` 在该供应商内唯一
4. `name` 尽量与 `id` 保持一致
5. 从 `docs/qwenCodeProviderPresets.ts` 提取 `contextWindowSize` 和 `maxOutputTokens`
6. 同步更新 `docs/qwenCodeProviderPresets.ts`（TS 文件是数据权威来源）

**示例：** 在 DeepSeek 供应商下添加 `deepseek-v3`：

```json
{
  "id": "deepseek-v3",
  "name": "deepseek-v3",
  "authType": ["openai"],
  "contextWindowSize": 131072,
  "maxOutputTokens": 65536
}
```

### 添加新供应商

1. 在 `provider-presets.json` 末尾追加新供应商对象
2. 填写必要字段：`name`、`baseUrl`、`envPrefix`、`models`
3. 如果 API 使用非标准鉴权头（非 `Authorization: Bearer`），设置 `authHeader`
4. 同步更新 `docs/qwenCodeProviderPresets.ts`

**示例：** 添加新供应商 "MyAPI"：

```json
{
  "name": "MyAPI",
  "baseUrl": "https://api.myapi.com/v1",
  "envPrefix": "MYAPI_API_KEY",
  "proxyMode": "direct",
  "billingType": "pay_per_use",
  "models": [
    {
      "id": "my-model-v1",
      "name": "my-model-v1",
      "authType": ["openai"],
      "contextWindowSize": 128000,
      "maxOutputTokens": 32000
    }
  ]
}
```

### 关键约束

| 约束 | 说明 |
|------|------|
| model ID 唯一性 | 同一供应商内 `models[].id` 不能重复 |
| `authType` 兼容性 | `openai` → OpenAI SDK 兼容端点；`anthropic` → Anthropic Messages API；`authType` 数组表示同一模型支持多种协议调用 |
| `baseUrl` 路径 | 代理层用此地址转发请求，需与 `authType` 的 SDK 兼容 |
| `authHeader` | 仅在非 `Authorization: Bearer` 时设置（如 Anthropic 的 `x-api-key`、Kimi 的 `api-key`） |
| `contextWindowSize` | 写入用户 settings.json 的 `generationConfig.contextWindowSize`，Qwen Code 据此决定上下文裁剪策略 |
| `maxOutputTokens` | 写入 `generationConfig.samplingParams.max_tokens`，控制单次最大输出 |

### 验证清单

更新预设后，运行以下验证：

```bash
# 1. JSON 格式校验
node -e "JSON.parse(require('fs').readFileSync('src-tauri/resources/provider-presets.json','utf8'))"

# 2. model ID 唯一性检查
node -e "
const d = JSON.parse(require('fs').readFileSync('src-tauri/resources/provider-presets.json','utf8'));
d.forEach(p => {
  const ids = p.models.map(m => m.id);
  const dupes = ids.filter((id,i) => ids.indexOf(id) !== i);
  if (dupes.length) console.error(p.name + ': 重复 ' + dupes);
});
console.log('✅ 检查完成');
"

# 3. Rust 编译 + 测试
cd src-tauri && cargo test
```

### Rust 结构体对应关系

| JSON 字段 | Rust 结构体 | 字段 |
|-----------|------------|------|
| 供应商对象 | `presets::ProviderPreset` | `name`, `base_url`, `env_prefix`, `models`, ... |
| 模型对象 | `presets::ModelPreset` | `id`, `name`, `auth_type`, `context_window_size`, `max_output_tokens` |

修改 JSON 字段时，需同步检查 `src-tauri/src/presets/mod.rs` 中的结构体定义是否匹配。
