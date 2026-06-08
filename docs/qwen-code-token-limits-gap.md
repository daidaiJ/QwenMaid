# Qwen Code tokenLimits.ts 预设规则覆盖不全的模型清单

> 基于 `packages/core/src/core/tokenLimits.ts` 的 `PATTERNS`（输入上下文窗口）和 `OUTPUT_PATTERNS`（最大输出）规则分析。
> 日期：2026-06-08

## 规则解析流程

```
model ID → normalize() → 正则匹配 → 命中则返回预设值 → 未命中则回退默认值
```

- 输入上下文窗口回退值：`DEFAULT_TOKEN_LIMIT` = **131,072**（128K）
- 最大输出回退值：`DEFAULT_OUTPUT_TOKEN_LIMIT` = **32,000**（32K）

## 一、输入上下文窗口（PATTERNS）

### 无匹配规则的模型

| 模型 | 归一化后 | 预计值 | 官方值 | 差距 |
|------|---------|--------|--------|------|
| MiMo-V2.5-Pro | `mimo-v2.5-pro` | 131,072 | 1,000,000 | 8.7x |
| MiMo-V2-Pro | `mimo-v2-pro` | 131,072 | 1,000,000 | 8.7x |
| MiMo-V2.5 | `mimo-v2.5` | 131,072 | 1,000,000 | 8.7x |
| MiMo-V2-Omni | `mimo-v2-omni` | 131,072 | 262,144 | 2x |
| MiMo-V2-Flash | `mimo-v2-flash` | 131,072 | 262,144 | 2x |

### 有匹配但值可能偏低的模型

| 模型 | 归一化后 | 匹配规则 | 预设值 | 备注 |
|------|---------|---------|--------|------|
| MiniMax M2.7 | `minimax-m2.7` | `minimax-` fallback | 200,000 | 走通用 fallback，无专属规则 |

### 有匹配且值正确的模型（参考）

| 模型 | 匹配规则 | 预设值 |
|------|---------|--------|
| MiniMax M3 | `^minimax-m3` | 1,000,000 |
| MiniMax M2.5 | `^minimax-m2\.5` | 196,608 |
| GLM-5 / GLM-5.1 | `^glm-5` | 202,752 |
| Kimi K2.5 / K2.6 | `^kimi-` | 262,144 |
| Qwen3.6/3.7 Plus/Max | `^qwen3\.\d` | 1,000,000 |
| DeepSeek V4 Pro/Flash | `^deepseek-v4` | 1,000,000 |

---

## 二、最大输出限制（OUTPUT_PATTERNS）

### 无匹配规则的模型（回退到 32K 默认值）

| 模型 | 归一化后 | 预设回退 | 官方值 | 差距 |
|------|---------|---------|--------|------|
| MiMo-V2.5-Pro | `mimo-v2.5-pro` | 32,000 | 128,000 | 4x |
| MiMo-V2.5 | `mimo-v2.5` | 32,000 | 128,000 | 4x |
| MiMo-V2-Flash | `mimo-v2-flash` | 32,000 | 64,000 | 2x |
| MiniMax M2.7 | `minimax-m2.7` | 32,000 | 196,608 | 6x |
| MiniMax M3 | `minimax-m3` | 32,000 | 512,000 | 16x |
| Kimi K2.6 | `kimi-k2.6` | 32,000 | 262,144 | 8x |

### 有匹配但值错误的模型

| 模型 | 归一化后 | 匹配规则 | 预设值 | 官方值 | 差距 |
|------|---------|---------|--------|--------|------|
| GLM-5 | `glm-5` | `^glm-5` | 16,384 | 131,072 | 8x |
| GLM-5.1 | `glm-5.1` | `^glm-5` | 16,384 | ≈131,072 | 8x |

### 有匹配且值正确的模型（参考）

| 模型 | 匹配规则 | 预设值 |
|------|---------|--------|
| MiniMax M2.5 | `^minimax-m2\.5` | 65,536 |
| Kimi K2.5 | `^kimi-k2\.5` | 32,768 |
| Qwen3.6/3.7 Plus/Max | `^qwen3\.\d` | 65,536 |
| DeepSeek V4 Pro/Flash | `^deepseek-v4` | 384,000 |

---

## 三、当前配置中的规避方式

以下模型已在 `settings.json` 的 `modelProviders` 中通过 `generationConfig.contextWindowSize` 显式设置了输入上下文窗口，绕过了预设规则：

| 模型 | 显式设置 | 实际输入 |
|------|---------|---------|
| deepseek-v4-flash | `contextWindowSize: 1000000` | 1,000,000 ✅ |
| deepseek-v4-pro | `contextWindowSize: 1000000` | 1,000,000 ✅ |
| mimo-v2.5-pro | `contextWindowSize: 1000000` | 1,000,000 ✅ |

**但最大输出（maxOutputTokens）没有显式配置**，仍然使用预设回退值 32K。

---

## 四、建议补充的规则

### PATTERNS（输入上下文窗口）

```typescript
// MiMo 系列
[/^mimo-v2\.\d-pro/, LIMITS['1m']],   // MiMo-V2.x-Pro: 1M
[/^mimo-v2\.\d$/, LIMITS['1m']],      // MiMo-V2.x (Omni/标准): 1M
[/^mimo-v2-omni/, LIMITS['256k']],    // MiMo-V2-Omni: 256K
[/^mimo-v2-flash/, LIMITS['256k']],   // MiMo-V2-Flash: 256K
[/^mimo/, LIMITS['128k']],            // MiMo fallback
```

### OUTPUT_PATTERNS（最大输出）

```typescript
// MiMo 系列
[/^mimo-v2\.\d-pro/, LIMITS['128k']], // MiMo-V2.x-Pro: 128K
[/^mimo-v2\.\d$/, LIMITS['128k']],    // MiMo-V2.x: 128K
[/^mimo-v2-flash/, LIMITS['64k']],    // MiMo-V2-Flash: 64K

// GLM 系列（修正值）
[/^glm-5/, LIMITS['128k']],           // GLM-5: 131,072（当前为 16K）

// MiniMax 系列
[/^minimax-m3/, LIMITS['512k']],      // MiniMax M3: 524,288
[/^minimax-m2\.7/, LIMITS['192k']],   // MiniMax M2.7: 196,608

// Kimi 系列
[/^kimi-k2\.6/, LIMITS['256k']],      // Kimi K2.6: 262,144
[/^kimi-/, LIMITS['256k']],           // Kimi fallback: 262,144
```

---

## 五、临时规避方案

在 `tokenLimits.ts` 补充规则之前，可通过以下方式手动覆盖：

### 方式一：provider 配置中设置 samplingParams（推荐）

```jsonc
{
  "id": "mimo-v2.5-pro",
  "generationConfig": {
    "contextWindowSize": 1000000,
    "samplingParams": {
      "max_tokens": 131072  // 直接指定，绕过 applyOutputTokenLimit
    }
  }
}
```

`samplingParams` 存在时，`applyOutputTokenLimit()` 直接返回原请求，不做任何 cap。

### 方式二：settings.json 全局 generationConfig

在 `settings.json` 的 `model.generationConfig` 中设置，对所有模型生效（当 provider 配置没有覆盖时）：

```jsonc
{
  "model": {
    "name": "mimo-v2.5-pro",
    "generationConfig": {
      "samplingParams": {
        "max_tokens": 131072
      }
    }
  }
}
```

字段解析优先级（高 → 低）：
1. `modelProviders[provider][model].generationConfig.samplingParams.max_tokens`
2. `model.generationConfig.samplingParams.max_tokens`
3. 环境变量 `QWEN_CODE_MAX_OUTPUT_TOKENS`
4. 预设规则 `tokenLimit(model, 'output')`
5. 默认值 `DEFAULT_OUTPUT_TOKEN_LIMIT`（32K）

### 方式三：环境变量

```bash
QWEN_CODE_MAX_OUTPUT_TOKENS=131072
```

对已知模型（有 OUTPUT_PATTERNS 匹配的）仍会被 cap 到模型上限；对未知模型（如 MiMo）直接生效。
