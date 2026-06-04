/**
 * Qwen Code 供应商预设配置
 *
 * 从 cc-switch opencodeProviderPresets 转换而来，筛选主流供应商。
 * 排除小中转站/聚合商（DMXAPI, PIHELLM, PackyCode, APIKEY.FUN,
 * APINebula, AtlasCloud, SudoCode, Cubence, TheRouter, Novita AI,
 * Shengsuanyun, AiHubMix 等）。
 *
 * 字段说明：
 * - authType: Qwen Code modelProviders 的 key（openai | anthropic | gemini）
 * - envKey:   环境变量名，实际 key 存放在 env 或 .env 文件中
 * - baseUrl:  API 端点，需与 authType 的 SDK 兼容
 */

// ─── 类型定义 ───────────────────────────────────────────────────────────────

export interface QwenCodeModelPreset {
  /** 模型 ID，即 modelProviders[].id */
  id: string;
  /** 显示名称 */
  name: string;
  /** 上下文窗口大小（tokens） */
  contextWindowSize?: number;
  /** 最大输出 tokens */
  maxOutputTokens?: number;
  /** 支持的输入模态 */
  inputModalities?: ("text" | "image" | "pdf" | "audio" | "video")[];
  /** 推理/思考配置 */
  reasoning?: { effort: "low" | "medium" | "high" | "max" };
  /** 额外采样参数 */
  samplingParams?: Record<string, unknown>;
  /** 额外请求体参数（仅 openai authType 生效） */
  extraBody?: Record<string, unknown>;
}

export interface QwenCodeProviderPreset {
  /** 供应商显示名称 */
  name: string;
  /** 供应商官网 */
  websiteUrl: string;
  /** API Key 获取页面 */
  apiKeyUrl?: string;
  /** 分类 */
  category: "official" | "cn_official" | "aggregator";
  /** Qwen Code authType（openai | anthropic | gemini） */
  authType: "openai" | "anthropic" | "gemini";
  /** API 端点 */
  baseUrl: string;
  /** 环境变量名 */
  envKey: string;
  /** 默认超时（ms） */
  timeout?: number;
  /** 预设模型列表 */
  models: QwenCodeModelPreset[];
  /** 图标标识 */
  icon?: string;
  /** 图标颜色 */
  iconColor?: string;
}

// ─── 预设数据 ───────────────────────────────────────────────────────────────

export const qwenCodeProviderPresets: QwenCodeProviderPreset[] = [
  // ═══════════════════════════════════════════════════════════════════════════
  // 国际大厂
  // ═══════════════════════════════════════════════════════════════════════════

  {
    name: "OpenAI",
    websiteUrl: "https://platform.openai.com",
    apiKeyUrl: "https://platform.openai.com/api-keys",
    category: "official",
    authType: "openai",
    baseUrl: "https://api.openai.com/v1",
    envKey: "OPENAI_API_KEY",
    timeout: 120000,
    icon: "openai",
    iconColor: "#10A37F",
    models: [
      {
        id: "gpt-5.5",
        name: "GPT-5.5",
        contextWindowSize: 400000,
        maxOutputTokens: 128000,
        inputModalities: ["text", "image"],
        reasoning: { effort: "high" },
      },
    ],
  },

  {
    name: "Anthropic",
    websiteUrl: "https://console.anthropic.com",
    apiKeyUrl: "https://console.anthropic.com/settings/keys",
    category: "official",
    authType: "anthropic",
    baseUrl: "https://api.anthropic.com/v1",
    envKey: "ANTHROPIC_API_KEY",
    timeout: 180000,
    icon: "anthropic",
    iconColor: "#D97757",
    models: [
      {
        id: "claude-opus-4-8",
        name: "Claude Opus 4.8",
        contextWindowSize: 1000000,
        maxOutputTokens: 128000,
        inputModalities: ["text", "image", "pdf"],
        reasoning: { effort: "high" },
      },
      {
        id: "claude-sonnet-4-5-20250929",
        name: "Claude Sonnet 4.5",
        contextWindowSize: 200000,
        maxOutputTokens: 64000,
        inputModalities: ["text", "image", "pdf"],
        reasoning: { effort: "medium" },
      },
      {
        id: "claude-haiku-4-5-20251001",
        name: "Claude Haiku 4.5",
        contextWindowSize: 200000,
        maxOutputTokens: 64000,
        inputModalities: ["text", "image", "pdf"],
      },
    ],
  },

  {
    name: "Google Gemini",
    websiteUrl: "https://aistudio.google.com",
    apiKeyUrl: "https://aistudio.google.com/apikey",
    category: "official",
    authType: "gemini",
    baseUrl: "https://generativelanguage.googleapis.com",
    envKey: "GEMINI_API_KEY",
    timeout: 120000,
    icon: "google",
    iconColor: "#4285F4",
    models: [
      {
        id: "gemini-3.5-flash",
        name: "Gemini 3.5 Flash",
        contextWindowSize: 1048576,
        maxOutputTokens: 65536,
        inputModalities: ["text", "image", "pdf", "video", "audio"],
      },
      {
        id: "gemini-2.5-flash-lite",
        name: "Gemini 2.5 Flash Lite",
        contextWindowSize: 1048576,
        maxOutputTokens: 65536,
        inputModalities: ["text", "image", "pdf", "video", "audio"],
      },
    ],
  },

  // ═══════════════════════════════════════════════════════════════════════════
  // 国内厂商（直连官方 API）
  // ═══════════════════════════════════════════════════════════════════════════

  {
    name: "DeepSeek",
    websiteUrl: "https://platform.deepseek.com",
    apiKeyUrl: "https://platform.deepseek.com/api_keys",
    category: "cn_official",
    authType: "openai",
    baseUrl: "https://api.deepseek.com/v1",
    envKey: "DEEPSEEK_API_KEY",
    timeout: 180000,
    icon: "deepseek",
    iconColor: "#1E88E5",
    models: [
      {
        id: "deepseek-v4-pro",
        name: "DeepSeek V4 Pro",
        contextWindowSize: 131072,
        maxOutputTokens: 65536,
        reasoning: { effort: "high" },
      },
      {
        id: "deepseek-v4-flash",
        name: "DeepSeek V4 Flash",
        contextWindowSize: 131072,
        maxOutputTokens: 65536,
      },
    ],
  },

  {
    name: "Zhipu GLM",
    websiteUrl: "https://open.bigmodel.cn",
    apiKeyUrl: "https://www.bigmodel.cn/claude-code",
    category: "cn_official",
    authType: "openai",
    baseUrl: "https://open.bigmodel.cn/api/coding/paas/v4",
    envKey: "ZHIPU_API_KEY",
    timeout: 120000,
    icon: "zhipu",
    iconColor: "#0F62FE",
    models: [
      {
        id: "glm-5.1",
        name: "GLM-5.1",
        contextWindowSize: 204800,
        maxOutputTokens: 131072,
      },
    ],
  },

  {
    name: "Bailian (阿里云百炼)",
    websiteUrl: "https://bailian.console.aliyun.com",
    apiKeyUrl: "https://bailian.console.aliyun.com/#/api-key",
    category: "cn_official",
    authType: "openai",
    baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    envKey: "DASHSCOPE_API_KEY",
    timeout: 120000,
    icon: "bailian",
    iconColor: "#624AFF",
    models: [
      {
        id: "qwen3-coder-plus",
        name: "Qwen3 Coder Plus",
        contextWindowSize: 131072,
        maxOutputTokens: 65536,
      },
      {
        id: "qwen3-max",
        name: "Qwen3 Max",
        contextWindowSize: 131072,
        maxOutputTokens: 65536,
      },
    ],
  },

  {
    name: "Kimi (月之暗面)",
    websiteUrl: "https://platform.moonshot.cn/console",
    apiKeyUrl: "https://platform.moonshot.cn/console/api-keys",
    category: "cn_official",
    authType: "openai",
    baseUrl: "https://api.moonshot.cn/v1",
    envKey: "MOONSHOT_API_KEY",
    timeout: 120000,
    icon: "kimi",
    iconColor: "#6366F1",
    models: [
      {
        id: "kimi-k2.6",
        name: "Kimi K2.6",
        contextWindowSize: 262144,
        maxOutputTokens: 262144,
        inputModalities: ["text", "image", "video"],
      },
    ],
  },

  {
    name: "Kimi For Coding",
    websiteUrl: "https://www.kimi.com/code/docs/",
    apiKeyUrl: "https://platform.moonshot.cn/console/api-keys",
    category: "cn_official",
    authType: "anthropic",
    baseUrl: "https://api.kimi.com/coding/v1",
    envKey: "KIMI_CODING_API_KEY",
    timeout: 120000,
    icon: "kimi",
    iconColor: "#6366F1",
    models: [
      {
        id: "kimi-for-coding",
        name: "Kimi For Coding",
        contextWindowSize: 262144,
        maxOutputTokens: 262144,
      },
    ],
  },

  {
    name: "StepFun (阶跃星辰)",
    websiteUrl: "https://platform.stepfun.com/step-plan",
    apiKeyUrl: "https://platform.stepfun.com/interface-key",
    category: "cn_official",
    authType: "openai",
    baseUrl: "https://api.stepfun.com/step_plan/v1",
    envKey: "STEPFUN_API_KEY",
    timeout: 120000,
    icon: "stepfun",
    iconColor: "#16D6D2",
    models: [
      {
        id: "step-3.5-flash-2603",
        name: "Step 3.5 Flash 2603",
        contextWindowSize: 262144,
      },
      {
        id: "step-3.5-flash",
        name: "Step 3.5 Flash",
        contextWindowSize: 262144,
      },
    ],
  },

  {
    name: "MiniMax",
    websiteUrl: "https://platform.minimaxi.com",
    apiKeyUrl: "https://platform.minimaxi.com/subscribe/coding-plan",
    category: "cn_official",
    authType: "openai",
    baseUrl: "https://api.minimaxi.com/v1",
    envKey: "MINIMAX_API_KEY",
    timeout: 120000,
    icon: "minimax",
    iconColor: "#FF6B6B",
    models: [
      {
        id: "MiniMax-M2.7",
        name: "MiniMax M2.7",
        contextWindowSize: 204800,
        maxOutputTokens: 131072,
      },
    ],
  },

  {
    name: "Volcengine Doubao (火山引擎豆包)",
    websiteUrl: "https://console.volcengine.com/ark",
    apiKeyUrl:
      "https://console.volcengine.com/ark/region:ark+cn-beijing/apiKey",
    category: "cn_official",
    authType: "openai",
    baseUrl: "https://ark.cn-beijing.volces.com/api/v3",
    envKey: "VOLCENGINE_API_KEY",
    timeout: 120000,
    icon: "doubao",
    iconColor: "#3370FF",
    models: [
      {
        id: "doubao-seed-2-0-code-preview-latest",
        name: "Doubao Seed Code Preview",
        contextWindowSize: 131072,
        maxOutputTokens: 65536,
      },
    ],
  },

  {
    name: "Volcengine AgentPlan (火山 Agentplan)",
    websiteUrl: "https://www.volcengine.com/activity/agentplan",
    apiKeyUrl: "https://console.volcengine.com/ark/apiKey",
    category: "cn_official",
    authType: "openai",
    baseUrl: "https://ark.cn-beijing.volces.com/api/coding/v3",
    envKey: "VOLCENGINE_AGENTPLAN_API_KEY",
    timeout: 120000,
    icon: "huoshan",
    iconColor: "#3370FF",
    models: [
      {
        id: "ark-code-latest",
        name: "Ark Code Latest",
        contextWindowSize: 131072,
      },
    ],
  },

  {
    name: "Xiaomi MiMo",
    websiteUrl: "https://platform.xiaomimimo.com",
    apiKeyUrl: "https://platform.xiaomimimo.com/#/console/api-keys",
    category: "cn_official",
    authType: "openai",
    baseUrl: "https://api.xiaomimimo.com/v1",
    envKey: "MIMO_API_KEY",
    timeout: 120000,
    icon: "xiaomimimo",
    iconColor: "#000000",
    models: [
      {
        id: "mimo-v2.5-pro",
        name: "MiMo V2.5 Pro",
        contextWindowSize: 1048576,
        maxOutputTokens: 131072,
      },
      {
        id: "mimo-v2.5",
        name: "MiMo V2.5",
        contextWindowSize: 1048576,
        maxOutputTokens: 131072,
        inputModalities: ["text", "image"],
      },
    ],
  },

  // ═══════════════════════════════════════════════════════════════════════════
  // 海外 / 第三方平台
  // ═══════════════════════════════════════════════════════════════════════════

  {
    name: "OpenCode Go",
    websiteUrl: "https://opencode.ai",
    apiKeyUrl: "https://opencode.ai",
    category: "aggregator",
    authType: "openai",
    baseUrl: "https://opencode.ai/zen/go/v1",
    envKey: "OPENCODE_API_KEY",
    timeout: 120000,
    icon: "opencode",
    iconColor: "#8B5CF6",
    models: [
      {
        id: "glm-5.1",
        name: "GLM-5.1 (via OpenCode)",
        contextWindowSize: 204800,
        maxOutputTokens: 131072,
      },
      {
        id: "kimi-k2.6",
        name: "Kimi K2.6 (via OpenCode)",
        contextWindowSize: 262144,
        maxOutputTokens: 262144,
      },
      {
        id: "kimi-k2.5",
        name: "Kimi K2.5 (via OpenCode)",
        contextWindowSize: 131072,
      },
      {
        id: "deepseek-v4-pro",
        name: "DeepSeek V4 Pro (via OpenCode)",
        contextWindowSize: 131072,
        maxOutputTokens: 65536,
        reasoning: { effort: "high" },
      },
      {
        id: "deepseek-v4-flash",
        name: "DeepSeek V4 Flash (via OpenCode)",
        contextWindowSize: 131072,
        maxOutputTokens: 65536,
      },
      {
        id: "mimo-v2.5",
        name: "MiMo V2.5 (via OpenCode)",
        contextWindowSize: 1048576,
        maxOutputTokens: 131072,
        inputModalities: ["text", "image"],
      },
      {
        id: "mimo-v2.5-pro",
        name: "MiMo V2.5 Pro (via OpenCode)",
        contextWindowSize: 1048576,
        maxOutputTokens: 131072,
      },
    ],
  },
  // OpenCode Anthropic 协议模型（MiniMax / Qwen 系列）
  {
    name: "OpenCode Go (Anthropic)",
    websiteUrl: "https://opencode.ai",
    apiKeyUrl: "https://opencode.ai",
    category: "aggregator",
    authType: "anthropic",
    baseUrl: "https://opencode.ai/zen/go/v1",
    envKey: "OPENCODE_API_KEY",
    timeout: 120000,
    icon: "opencode",
    iconColor: "#8B5CF6",
    models: [
      {
        id: "minimax-m3",
        name: "MiniMax M3 (via OpenCode)",
        contextWindowSize: 204800,
        maxOutputTokens: 131072,
      },
      {
        id: "minimax-m2.7",
        name: "MiniMax M2.7 (via OpenCode)",
        contextWindowSize: 204800,
        maxOutputTokens: 131072,
      },
      {
        id: "qwen3.7-max",
        name: "Qwen3.7 Max (via OpenCode)",
        contextWindowSize: 131072,
      },
      {
        id: "qwen3.6-plus",
        name: "Qwen3.6 Plus (via OpenCode)",
        contextWindowSize: 131072,
      },
    ],
  },

  {
    name: "Nvidia NIM",
    websiteUrl: "https://build.nvidia.com",
    apiKeyUrl: "https://build.nvidia.com/settings/api-keys",
    category: "official",
    authType: "openai",
    baseUrl: "https://integrate.api.nvidia.com/v1",
    envKey: "NVIDIA_API_KEY",
    timeout: 120000,
    icon: "nvidia",
    iconColor: "#76B900",
    models: [
      {
        id: "moonshotai/kimi-k2.5",
        name: "Kimi K2.5 (via Nvidia NIM)",
        contextWindowSize: 131072,
      },
    ],
  },

  {
    name: "Astron 讯飞星火",
    websiteUrl: "https://xinghuo.xfyun.cn",
    apiKeyUrl: "https://console.xfyun.cn",
    category: "cn_official",
    authType: "openai",
    baseUrl: "https://maas-token-api.cn-huabei-1.xf-yun.com/v2",
    envKey: "XFYUN_API_KEY",
    timeout: 120000,
    icon: "xfyun",
    iconColor: "#0066FF",
    models: [
      {
        id: "xsparkx2",
        name: "Spark X2",
        contextWindowSize: 131072,
      },
      {
        id: "xsparkx2flash",
        name: "Spark X2 Flash",
        contextWindowSize: 131072,
      },
      {
        id: "xopglm51",
        name: "GLM-5.1 (讯飞)",
        contextWindowSize: 131072,
      },
      {
        id: "xopdeepseekv4pro",
        name: "DeepSeek V4 Pro (讯飞)",
        contextWindowSize: 131072,
      },
      {
        id: "xopkimik26",
        name: "Kimi K2.6 (讯飞)",
        contextWindowSize: 262144,
      },
      {
        id: "xopqwen35397b",
        name: "Qwen3.5-397B (讯飞)",
        contextWindowSize: 131072,
      },
      {
        id: "xop3qwencodernext",
        name: "Qwen3-Coder-Next (讯飞)",
        contextWindowSize: 131072,
      },
    ],
  },

  // ═══════════════════════════════════════════════════════════════════════════
  // 聚合平台
  // ═══════════════════════════════════════════════════════════════════════════

  {
    name: "OpenRouter",
    websiteUrl: "https://openrouter.ai",
    apiKeyUrl: "https://openrouter.ai/keys",
    category: "aggregator",
    authType: "openai",
    baseUrl: "https://openrouter.ai/api/v1",
    envKey: "OPENROUTER_API_KEY",
    timeout: 120000,
    icon: "openrouter",
    iconColor: "#6566F1",
    models: [
      {
        id: "anthropic/claude-opus-4.8",
        name: "Claude Opus 4.8 (via OpenRouter)",
        contextWindowSize: 1000000,
        maxOutputTokens: 128000,
      },
      {
        id: "anthropic/claude-sonnet-4.6",
        name: "Claude Sonnet 4.6 (via OpenRouter)",
        contextWindowSize: 200000,
        maxOutputTokens: 64000,
      },
      {
        id: "openai/gpt-5.5",
        name: "GPT-5.5 (via OpenRouter)",
        contextWindowSize: 400000,
        maxOutputTokens: 128000,
      },
      {
        id: "google/gemini-3.5-flash",
        name: "Gemini 3.5 Flash (via OpenRouter)",
        contextWindowSize: 1048576,
        maxOutputTokens: 65536,
      },
      {
        id: "qwen/qwen3-coder-480b",
        name: "Qwen3 Coder 480B (via OpenRouter)",
        contextWindowSize: 262144,
      },
    ],
  },

  {
    name: "ModelScope (魔搭社区)",
    websiteUrl: "https://modelscope.cn",
    apiKeyUrl: "https://modelscope.cn/my/myaccesstoken",
    category: "aggregator",
    authType: "openai",
    baseUrl: "https://api-inference.modelscope.cn/v1",
    envKey: "MODELSCOPE_API_KEY",
    timeout: 120000,
    icon: "modelscope",
    iconColor: "#624AFF",
    models: [
      {
        id: "ZhipuAI/GLM-5.1",
        name: "GLM-5.1 (via ModelScope)",
        contextWindowSize: 204800,
        maxOutputTokens: 131072,
      },
    ],
  },
];

// ─── 辅助函数 ───────────────────────────────────────────────────────────────

/**
 * 按 authType 分组，生成 Qwen Code settings.json 的 modelProviders 片段
 */
export function generateModelProvidersConfig(
  presets: QwenCodeProviderPreset[] = qwenCodeProviderPresets,
): Record<string, Array<Record<string, unknown>>> {
  const result: Record<string, Array<Record<string, unknown>>> = {};

  for (const preset of presets) {
    if (!result[preset.authType]) {
      result[preset.authType] = [];
    }

    for (const model of preset.models) {
      const entry: Record<string, unknown> = {
        id: model.id,
        name: model.name,
        envKey: preset.envKey,
        baseUrl: preset.baseUrl,
      };

      const genConfig: Record<string, unknown> = {};
      if (preset.timeout) genConfig.timeout = preset.timeout;
      if (model.contextWindowSize)
        genConfig.contextWindowSize = model.contextWindowSize;
      if (model.maxOutputTokens)
        genConfig.samplingParams = {
          ...(genConfig.samplingParams as Record<string, unknown>),
          max_tokens: model.maxOutputTokens,
        };
      if (model.inputModalities) {
        genConfig.modalities = Object.fromEntries(
          model.inputModalities.map((m) => [m, true]),
        );
      }
      if (model.reasoning) genConfig.reasoning = model.reasoning;
      if (model.samplingParams)
        genConfig.samplingParams = {
          ...(genConfig.samplingParams as Record<string, unknown>),
          ...model.samplingParams,
        };
      if (model.extraBody) genConfig.extra_body = model.extraBody;

      if (Object.keys(genConfig).length > 0) {
        entry.generationConfig = genConfig;
      }

      result[preset.authType]!.push(entry);
    }
  }

  return result;
}

/**
 * 生成完整的 settings.json 片段（含 env 和 modelProviders）
 */
export function generateSettingsSnippet(
  presets: QwenCodeProviderPreset[] = qwenCodeProviderPresets,
): Record<string, unknown> {
  const env: Record<string, string> = {};
  for (const preset of presets) {
    env[preset.envKey] = "";
  }

  return {
    env,
    modelProviders: generateModelProvidersConfig(presets),
  };
}
