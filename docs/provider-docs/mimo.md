支持两种使用方式，但对应的凭证获取方式不同：

使用方式	说明	获取方式（以下为 BASE_URL 和 API Key 均为示例）
按量付费 API 调用	按实际使用量计费，适合轻度使用	
BASE_URL
OpenAI 兼容协议：https://api.xiaomimimo.com/v1
Anthropic 兼容协议：https://api.xiaomimimo.com/anthropic
API Key
格式：sk-xxxxx

前往 API Keys 创建 API Key
Token Plan	固定订阅费，按套餐限量调用	
BASE_URL
OpenAI 兼容协议：https://token-plan-cn.xiaomimimo.com/v1
Anthropic 兼容协议：https://token-plan-cn.xiaomimimo.com/anthropic
API Key
格式：tp-xxxxx

-----

模型系列	模型 ID (Model ID)	能力支持	长度限制（token）	限流
Pro 系列	mimo-v2.5-pro	文本生成
上下文窗口：1M
最大输出：128K	最大 RPM：100
最大 TPM：10M

