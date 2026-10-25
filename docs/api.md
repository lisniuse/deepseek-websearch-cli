# DeepSeek 网页搜索与抓取 API 摘要

> 来源：<https://api-docs.deepseek.com/zh-cn/guides/search/>、<https://api-docs.deepseek.com/zh-cn/api/search>、<https://api-docs.deepseek.com/zh-cn/api/fetch>、<https://api-docs.deepseek.com/zh-cn/quick_start/pricing>
>
> 本文件是对官方文档的摘录整理，供本 CLI 实现参考。

## 概述

- **Search**：按 `query` 搜索网页，返回结构化结果列表。由 DeepSeek 自研搜索引擎支持。
- **Fetch**：抓取单个网页，以 Markdown 文本返回页面内容。
- 两者都是原始 JSON API，使用与 DeepSeek API 相同的 Bearer 鉴权。
- Base URL：`https://api.deepseek.com`

| API | Endpoint |
| --- | --- |
| Search | `POST https://api.deepseek.com/v1/tools/search` |
| Fetch | `POST https://api.deepseek.com/v1/tools/fetch` |

## 鉴权

| 请求头 | 必填 | 值 |
| --- | --- | --- |
| `Authorization` | 是 | `Bearer <DeepSeek API Key>` |
| `Content-Type` | 是 | `application/json` |

## Search 接口

### 请求字段

| 字段 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `query` | string | 是 | - | 搜索词，1～128 个 Unicode 字符，不能全为空白，支持下方搜索语法 |
| `count` | integer | 否 | 10 | 返回条数，1～50 |
| `ori_question` | string | 否 | - | 触发搜索的用户原始问题，1～128 个字符，有助于理解搜索意图，传 `null` 视为未提供 |
| `localization` | string | 否 | `""` | 用于优先返回本地化搜索结果的 IP 地址 |
| `need_content` | boolean | 否 | false | 是否在 `content` 字段返回网页正文，为 false 时 `content` 恒为空字符串 |

请求体中的未知字段会被忽略。

### 搜索语法

`query` 中的语法会原样传递给搜索服务，可在一条 query 中组合多个操作符。

| 操作符 | 示例 | 说明 |
| --- | --- | --- |
| `site:` | `DeepSeek site:github.com` | 限定站点；指定多个时用空格分隔并通过 `OR` 连接，如 `DeepSeek site:github.com OR site:huggingface.co` |
| `-site:` | `DeepSeek -site:example.com` | 排除站点；指定多个时用空格分隔 |
| `-` | `DeepSeek -广告` | 排除含指定关键词的网页；指定多个时用空格分隔 |
| `-"..."` | `DeepSeek -"release notes"` | 排除含指定短语的网页，短语内可含空格 |
| `"..."` | `DeepSeek "精确短语"` | 精确匹配短语 |
| `lang:` | `DeepSeek lang:zh,en` | 限定语言，逗号分隔 |
| `after:` | `DeepSeek after:2025` / `after:2025-01-01` | 限定发布日期之后 |
| `before:` | `DeepSeek before:2026` / `before:2026-06-01` | 限定发布日期之前 |

### 响应

HTTP 200，返回 `results` 数组；搜索正常结束但没有匹配结果时返回空数组。

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `rerank_score` | number | 排序分数，0～3，综合考虑相关性、权威性、时效性；>= 1.5 为优质结果 |
| `title` | string | 标题 |
| `url` | string | 结果链接 |
| `snippet` | string | 短摘要，与 query 相关，通常 100～200 字符 |
| `summary` | string | 长摘要，与 query 相关，通常 600～800 字符，适合给模型阅读 |
| `content` | string | 网页正文；`need_content` 为 false 或正文缺失时为空字符串 |
| `site_name` | string \| null | 站点名称，缺失时为 null |
| `site_icon` | string \| null | 站点图标 URL，缺失时为 null |
| `date` | string \| null | 发布时间，统一为 UTC 的 RFC 3339 格式，未知时为 null |
| `authority` | integer | 网页权威性 0～4（-1 表示未知）；>= 3 为极权威结果 |

示例响应：

```json
{
  "results": [
    {
      "rerank_score": 2.4,
      "title": "DeepSeek V4 模型发布",
      "url": "https://example.com/deepseek-v4",
      "snippet": "DeepSeek V4 已上线，支持工具调用与长上下文。",
      "summary": "DeepSeek 发布了 V4 模型。此次发布改进了工具调用与长上下文能力，适合 Agent 工作流。",
      "content": "DeepSeek V4 现已正式上线……",
      "site_name": "Example News",
      "site_icon": "https://example.com/favicon.ico",
      "date": "2026-06-01T00:00:00+00:00",
      "authority": 3
    }
  ]
}
```

## Fetch 接口

### 请求字段

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `url` | string | 是 | 要抓取的网页地址，长度不超过 8192，必须为含 host 的 HTTP/HTTPS URL，且不能携带用户名或密码 |

### 响应

```json
{
  "title": "Example article",
  "content": "# Example article\n\nArticle content."
}
```

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `title` | string | 页面标题，缺失时为空字符串 |
| `content` | string | 抓取后的 Markdown 正文 |

Fetch 的结果受目标站点的可用性和访问策略影响。

## 错误

错误响应使用 OpenAI 兼容的错误格式，`param` 字段恒为 `null`：

```json
{
  "error": {
    "message": "'count' must be between 1 and 50",
    "type": "invalid_request_error",
    "param": null,
    "code": "invalid_request_error"
  }
}
```

| HTTP 状态码 | 说明 | 是否重试 |
| --- | --- | --- |
| 400 | 请求头或请求体不合法（JSON 格式错误或字段值无效） | 否 |
| 401 | API Key 缺失或无效 | 否 |
| 402 | 账号余额不足 | 否（需充值） |
| 403 | Fetch 请求被安全策略拒绝 | 否 |
| 404 | Fetch 未能获取可用的网页内容（URL 不可用、被站点策略拦截、目标站点返回错误、内容类型不受支持或页面没有可提取的 Markdown 正文） | 否 |
| 408 | 读取请求体超时 | 是 |
| 413 | 请求体超过大小限制 | 否 |
| 429 | 超过 Search 或 Fetch 的 QPS 限制 | 是 |
| 500 | 服务器内部错误 | 是 |
| 504 | 下游请求超时 | 是 |

官方建议：遇到 408、429 或 5xx 错误时等待后重试；遇到 402 请先充值；其它 4xx 错误在不修改请求的前提下不要重试。

## 计费

Search 和 Fetch 按**请求次数**计费，不按 token 或返回结果条数计费。

| API | 计费单位 | 价格 |
| --- | --- | --- |
| Search | 每 1,000 次成功请求 | 5 元 |
| Fetch | 每 1,000 次成功请求 | 5 元 |

只有返回 HTTP 200 且产生结果的请求才计费（Search 的 `results` 必须非空，Fetch 必须返回网页正文）。请求失败、搜索无结果或超时不计费。`count` 只控制返回条数，不影响单次请求价格。

> 通过 DeepSeek Harness / Claude Code / Codex 中经 Server 端工具调用触发的 Search/Fetch，自北京时间 **2026-11-01 00:00** 起按次计费。

## 限速

按账号粒度计算，与使用哪个 API Key 无关，与模型并发限制相互独立；Search 与 Fetch 分别独立限速。

| API | QPS 限制 |
| --- | --- |
| Search | 100 |
| Fetch | 25 |

超出账号级 QPS 限制会收到 HTTP 429。Fetch 还针对目标域名有独立的 QPS 限制，超限同样返回 429。
