import { invoke } from "@tauri-apps/api/core";

// ── Types ────────────────────────────────────────────────

export interface Provider {
  id: number;
  name: string;
  base_url: string;
  api_key_env: string;
  proxy_mode: "system" | "custom" | "direct";
  proxy_url: string | null;
  auth_header: string | null;
  billing_type: "plan" | "pay_per_use";
  is_active: boolean;
  compress_enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface Model {
  id: number;
  provider_id: number;
  model_id: string;
  display_name: string | null;
  auth_type: string;
  is_default: boolean;
  config_json: string | null;
  created_at: string;
}

// ── Provider Commands ────────────────────────────────────

export const listProviders = () => invoke<Provider[]>("list_providers");

export const getProvider = (id: number) =>
  invoke<Provider>("get_provider", { id });

export const createProvider = (args: {
  name: string;
  baseUrl: string;
  apiKeyEnv: string;
  proxyMode?: string;
  proxyUrl?: string;
  authHeader?: string;
  billingType?: string;
  compressEnabled?: boolean;
}) => invoke<Provider>("create_provider", args);

export const updateProvider = (args: {
  id: number;
  name?: string;
  baseUrl?: string;
  apiKeyEnv?: string;
  proxyMode?: string;
  proxyUrl?: string;
  authHeader?: string;
  billingType?: string;
  isActive?: boolean;
  compressEnabled?: boolean;
}) => invoke<Provider>("update_provider", args);

export const deleteProvider = (id: number) =>
  invoke<void>("delete_provider", { id });

// ── Model Commands ───────────────────────────────────────

export const listModels = (providerId?: number) =>
  invoke<Model[]>("list_models", { providerId: providerId ?? null });

export const getModel = (id: number) => invoke<Model>("get_model", { id });

export const createModel = (args: {
  providerId: number;
  modelId: string;
  displayName?: string;
  authType: string;
  isDefault?: boolean;
  configJson?: string;
}) => invoke<Model>("create_model", args);

export const updateModel = (args: {
  id: number;
  displayName?: string;
  authType?: string;
  isDefault?: boolean;
  configJson?: string;
}) => invoke<Model>("update_model", args);

export const deleteModel = (id: number) =>
  invoke<void>("delete_model", { id });

// ── Config Commands ──────────────────────────────────────

export const readSettings = () =>
  invoke<Record<string, unknown>>("read_settings");

export const writeSettingsField = (path: string, value: unknown) =>
  invoke<Record<string, unknown>>("write_settings_field", { path, value });

export const getEnvVars = () =>
  invoke<Record<string, string>>("get_env_vars");

export const syncConfigToSettings = () =>
  invoke<Record<string, unknown>>("sync_config_to_settings");

export const previewSyncConfig = () =>
  invoke<Record<string, unknown>>("preview_sync_config");

// ── Provider Discovery ───────────────────────────────────

export interface DiscoveredModel {
  id: string;
  name: string;
  auth_type: string[];
  config_json: string | null;
  valid: boolean;
  from_preset: boolean;
}

export interface DiscoveredProvider {
  name: string;
  base_url: string;
  protocol: string;
  env_key: string;
  has_key: boolean;
  is_preset: boolean;
  preset_name: string | null;
  models: DiscoveredModel[];
  valid: boolean;
  error: string | null;
}

export const discoverExistingProviders = () =>
  invoke<DiscoveredProvider[]>("discover_existing_providers");

export const syncPresetModelsToSettings = () =>
  invoke<number>("sync_preset_models_to_settings");

// ── File System Commands ─────────────────────────────────

export const revealInExplorer = (path: string) =>
  invoke<void>("reveal_in_explorer", { path });

export const getQwenPaths = () =>
  invoke<Record<string, string>>("get_qwen_paths");

// ── Installer Commands ───────────────────────────────────

export interface ToolVersion {
  path: string;
  version: string;
}

export const detectQwenVersion = () =>
  invoke<string | null>("detect_qwen_version");

export const checkLatestQwenVersion = () =>
  invoke<string>("check_latest_qwen_version");

export const detectNodeVersion = () =>
  invoke<ToolVersion | null>("detect_node_version");

export const detectNpmVersion = () =>
  invoke<ToolVersion | null>("detect_npm_version");

export const installQwenCode = (mirror?: string) =>
  invoke<string>("install_qwen_code", { mirror: mirror ?? null });

export const updateQwenCode = (mirror?: string) =>
  invoke<string>("update_qwen_code", { mirror: mirror ?? null });

export const configureNpmMirror = (registry: string) =>
  invoke<void>("configure_npm_mirror", { registry });

export const getNpmMirror = () =>
  invoke<string>("get_npm_mirror");

// ── Filesystem Commands ──────────────────────────────────

export interface SkillInfo {
  name: string;
  description: string;
  skill_type: string;
  path: string;
  source: string;
}

export interface ProjectInfo {
  name: string;
  path: string;
  session_count: number;
}

export interface SessionInfo {
  id: string;
  title: string;
  started_at: string;
  message_count: number;
  input_tokens: number;
  file_path: string;
}

export interface ToolCallStat {
  name: string;
  count: number;
}

export interface SessionMessage {
  uuid: string;
  msg_type: string;
  timestamp: string;
  model: string | null;
  text: string;
  thinking: string | null;
  has_tool_use: boolean;
  tool_name: string | null;
  tool_input_preview: string | null;
  input_tokens: number;
  output_tokens: number;
}

export interface SessionDetail {
  message_count: number;
  models: string;
  input_tokens: number;
  output_tokens: number;
  duration: string;
  tool_calls: ToolCallStat[];
  skill_calls: ToolCallStat[];
  agent_calls: ToolCallStat[];
}

export interface PagedMessages {
  messages: SessionMessage[];
  total_count: number;
  has_older: boolean;
  has_newer: boolean;
}

export interface MemoryFile {
  name: string;
  memory_type: string;
  description: string;
  path: string;
}

export interface AgentDef {
  name: string;
  description: string;
  model: string;
  path: string;
}

export interface ExtensionInfo {
  name: string;
  version: string;
  description: string;
  enabled: boolean;
  path: string;
  has_skills: boolean;
  has_hooks: boolean;
  has_commands: boolean;
  has_agents: boolean;
}

// Skills
export const listSkills = () => invoke<SkillInfo[]>("list_skills");
export const readSkillContent = (path: string) => invoke<string>("read_skill_content", { path });
export const writeSkill = (path: string, content: string) =>
  invoke<void>("write_skill", { path, content });
export const deleteSkill = (path: string) => invoke<void>("delete_skill", { path });

// Projects & Sessions
export const listProjects = () => invoke<ProjectInfo[]>("list_projects");
export const listSessions = (project: string, limit?: number) =>
  invoke<SessionInfo[]>("list_sessions", { project, limit: limit ?? null });
export const getSessionDetail = (project: string, sessionId: string) =>
  invoke<SessionDetail>("get_session_detail", { project, sessionId });
export const getSessionMessagesPaged = (project: string, sessionId: string, offset: number, limit: number) =>
  invoke<PagedMessages>("get_session_messages_paged", { project, sessionId, offset, limit });
export const readSession = (project: string, sessionId: string) =>
  invoke<unknown[]>("read_session", { project, sessionId });

// Memory
export const listMemories = (project?: string) =>
  invoke<MemoryFile[]>("list_memories", { project: project ?? null });
export const readMemory = (path: string) =>
  invoke<{ frontmatter: string; content: string }>("read_memory", { path });
export const writeMemory = (path: string, content: string) =>
  invoke<void>("write_memory", { path, content });
export const deleteMemory = (path: string) => invoke<void>("delete_memory", { path });

// Index
export interface GlobalIndex {
  memories: MemoryFile[];
  projects: ProjectIndex[];
}
export interface ProjectIndex {
  name: string;
  session_count: number;
  valid_session_count: number;
  memory_count: number;
  latest_session: string | null;
}
export const getIndex = (limit?: number, offset?: number) =>
  invoke<GlobalIndex>("get_index", { limit: limit ?? null, offset: offset ?? null });

// Analytics
export interface NameCount {
  name: string;
  count: number;
}
export interface ModelRanking {
  name: string;
  session_count: number;
  input_tokens: number;
  output_tokens: number;
  cache_read: number;
  cache_hit_rate: number;
}
export interface ProjectStats {
  project: string;
  session_count: number;
  total_messages: number;
  total_tokens: number;
}
export interface DailyStats {
  date: string;
  session_count: number;
  message_count: number;
  input_tokens: number;
  output_tokens: number;
}
export interface ModelDailyRow {
  date: string;
  model: string;
  input_tokens: number;
  output_tokens: number;
  cache_read: number;
  message_count: number;
}
export interface AnalyticsSummary {
  total_sessions: number;
  total_messages: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_cache_read: number;
  active_days: number;
  top_models: ModelRanking[];
  project_stats: ProjectStats[];
  daily: DailyStats[];
  model_daily: ModelDailyRow[];
}
export interface AnalyticsTopItems {
  top_tools: NameCount[];
  top_skills: NameCount[];
  top_agents: NameCount[];
}
export const syncSessionStats = () => invoke<number>("sync_session_stats");
export const getAnalyticsSummary = () =>
  invoke<AnalyticsSummary>("get_analytics_summary");
export const getAnalyticsTopItems = () =>
  invoke<AnalyticsTopItems>("get_analytics_top_items");

// ── Model Detail (usage.db + proxy) ──────────────────────

export interface ModelMeta {
  model: string;
  total_requests: number;
  total_input: number;
  total_output: number;
  total_cache: number;
  avg_tps: number;
  p50_latency: number;
  p95_latency: number;
}

export interface ModelDailyDetail {
  date: string;
  model: string;
  input_tokens: number;
  output_tokens: number;
  cache_read: number;
  uncached_input: number;
  avg_tps: number;
  avg_latency: number;
  p50_latency: number;
  p95_latency: number;
  request_count: number;
}

export interface ModelDetailData {
  models: ModelMeta[];
  daily: ModelDailyDetail[];
}

export interface UsageDbInfo {
  exists: boolean;
  tables: string[];
  call_records_columns: string[];
  call_records_count: number;
  sample_row: string | null;
}

export const checkUsageDb = () => invoke<UsageDbInfo>("check_usage_db");
export const getModelDetailStats = (days: number) =>
  invoke<ModelDetailData>("get_model_detail_stats", { days });
export const getProxyDetailStats = (days: number) =>
  invoke<ModelDetailData>("get_proxy_detail_stats", { days });

// Agents
export const listAgents = () => invoke<AgentDef[]>("list_agents");
export const readAgent = (name: string) =>
  invoke<{ frontmatter: string; content: string }>("read_agent", { name });
export const writeAgent = (name: string, content: string) =>
  invoke<void>("write_agent", { name, content });
export const deleteAgent = (name: string) => invoke<void>("delete_agent", { name });

// Extensions
export const listExtensions = () => invoke<ExtensionInfo[]>("list_extensions");
export const readExtensionDetail = (name: string) =>
  invoke<{ config: unknown; context: string | null; path: string }>("read_extension_detail", { name });
export const toggleExtension = (name: string, enabled: boolean) =>
  invoke<void>("toggle_extension", { name, enabled });
export const deleteExtension = (name: string) => invoke<void>("delete_extension", { name });
export const writeExtensionContext = (name: string, content: string) =>
  invoke<void>("write_extension_context", { name, content });

// ── MCP Commands ─────────────────────────────────────────

export interface McpConfig {
  port: number;
  auto_inject: boolean;
  smartsearch_enabled: boolean;
  academicsearch_enabled: boolean;
  cleanfetch_enabled: boolean;
  search_mode: string;
  tavily_api_key: string | null;
  baidu_api_key: string | null;
  jina_api_key: string | null;
  proxy_url: string | null;
}

export interface ToolStats {
  tool_name: string;
  total: number;
  success: number;
}

export interface ApiStats {
  api_name: string;
  total: number;
  success: number;
}

export interface McpStats {
  monthly_total: number;
  monthly_success: number;
  by_tool: ToolStats[];
  by_api: ApiStats[];
}

export const getMcpConfig = () => invoke<McpConfig>("get_mcp_config");

export const saveMcpConfig = (config: McpConfig) =>
  invoke<void>("save_mcp_config", {
    port: config.port,
    autoInject: config.auto_inject,
    smartsearchEnabled: config.smartsearch_enabled,
    academicsearchEnabled: config.academicsearch_enabled,
    cleanfetchEnabled: config.cleanfetch_enabled,
    searchMode: config.search_mode,
    tavilyApiKey: config.tavily_api_key,
    baiduApiKey: config.baidu_api_key,
    jinaApiKey: config.jina_api_key,
    proxyUrl: config.proxy_url,
  });

export const restartMcpServer = () => invoke<void>("restart_mcp_server");

export const getMcpStatus = () => invoke<boolean>("get_mcp_status");

export const getMcpStats = () => invoke<McpStats>("get_mcp_stats");

export const injectStatusline = () => invoke<void>("inject_statusline");

export const removeStatusline = () => invoke<void>("remove_statusline");

export const checkUsageAutostart = () => invoke<boolean>("check_usage_autostart");

export const setUsageAutostart = (enable: boolean) => invoke<void>("set_usage_autostart", { enable });

// ── Proxy Status Commands ────────────────────────────────

export interface ProxyStatus {
  running: boolean;
  port: number;
  uptime_hint: string;
}

export interface ProviderModelStats {
  provider_id: number;
  provider_name: string;
  base_url: string;
  model_id: string;
  call_count: number;
  success_count: number;
  failure_count: number;
  total_input_tokens: number;
  total_output_tokens: number;
  avg_duration_ms: number;
  total_tokens_saved: number;
  compressed_count: number;
}

export interface ProxyProviderStats {
  providers: ProviderModelStats[];
  total_calls: number;
  total_failures: number;
  total_tokens_saved: number;
}

export const getProxyStatus = () => invoke<ProxyStatus>("get_proxy_status");

export const getProxyProviderStats = (days?: number) =>
  invoke<ProxyProviderStats>("get_proxy_provider_stats", {
    days: days ?? null,
  });

export const resetProviderCounts = (providerId: number) =>
  invoke<number>("reset_provider_counts", { providerId });
