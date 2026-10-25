//! DeepSeek Search / Fetch API 的 HTTP 客户端。

use std::time::Duration;

use anyhow::{bail, Context, Result};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::model::{ApiErrorEnvelope, FetchRequest, FetchResponse, SearchRequest, SearchResponse};

/// 默认 Base URL。
pub const DEFAULT_BASE_URL: &str = "https://api.deepseek.com";

/// 搜索接口路径。
pub const SEARCH_PATH: &str = "/v1/tools/search";
/// 抓取接口路径。
pub const FETCH_PATH: &str = "/v1/tools/fetch";

/// API 客户端。
#[derive(Debug)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    max_retries: u32,
}

impl Client {
    /// 构造客户端。
    ///
    /// - `base_url` 末尾的 `/` 会被去掉。
    /// - `max_retries` 针对 408 / 429 / 5xx 生效（官方建议这些错误等待后重试）。
    pub fn new(
        base_url: impl Into<String>,
        api_key: &str,
        timeout: Duration,
        max_retries: u32,
    ) -> Result<Self> {
        if api_key.trim().is_empty() {
            bail!("API Key 不能为空");
        }
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {api_key}"))
                .context("API Key 含有非法字符")?,
        );
        headers.insert(
            CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(timeout)
            .build()
            .context("创建 HTTP 客户端失败")?;

        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            max_retries,
        })
    }

    /// 当前使用的 Base URL。
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 调用网页搜索接口。
    pub async fn search(&self, request: &SearchRequest) -> Result<SearchResponse> {
        self.post_json(SEARCH_PATH, request).await
    }

    /// 调用网页抓取接口。
    pub async fn fetch(&self, url: &str) -> Result<FetchResponse> {
        self.post_json(
            FETCH_PATH,
            &FetchRequest {
                url: url.to_string(),
            },
        )
        .await
    }

    /// 发送 POST JSON 请求并解析响应。
    async fn post_json<T: DeserializeOwned>(&self, path: &str, body: &impl Serialize) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let payload = serde_json::to_vec(body).context("序列化请求体失败")?;

        let mut attempt: u32 = 0;
        loop {
            let response = self
                .http
                .post(&url)
                .body(payload.clone())
                .send()
                .await
                .with_context(|| format!("请求 {url} 失败"))?;

            let status = response.status();
            if status.is_success() {
                let text = response.text().await.context("读取响应体失败")?;
                return serde_json::from_str(&text).with_context(|| {
                    format!("解析响应 JSON 失败，原始内容: {}", truncate(&text, 300))
                });
            }

            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            let body_text = response.text().await.unwrap_or_default();
            let detail = describe_error(status, &body_text);

            if is_retryable(status) && attempt < self.max_retries {
                attempt += 1;
                let wait = retry_after
                    .map(Duration::from_secs)
                    .unwrap_or_else(|| Duration::from_millis(500 * 2u64.pow(attempt - 1)));
                tracing::warn!(
                    status = status.as_u16(),
                    attempt,
                    wait_ms = wait.as_millis() as u64,
                    error = %detail,
                    "请求失败，准备重试"
                );
                tokio::time::sleep(wait).await;
                continue;
            }

            bail!("{detail}");
        }
    }
}

/// 408 / 429 / 5xx 可重试。
fn is_retryable(status: StatusCode) -> bool {
    status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

/// 把 HTTP 状态码与错误体转换成可读信息。
fn describe_error(status: StatusCode, body: &str) -> String {
    let detail = match serde_json::from_str::<ApiErrorEnvelope>(body) {
        Ok(env) if env.error.message.is_some() || env.error.code.is_some() => env.error.describe(),
        _ => truncate(body, 300),
    };
    let hint = match status {
        StatusCode::BAD_REQUEST => "请求头或请求体不合法",
        StatusCode::UNAUTHORIZED => "API Key 缺失或无效",
        StatusCode::PAYMENT_REQUIRED => "账号余额不足，请充值",
        StatusCode::FORBIDDEN => "请求被安全策略拒绝",
        StatusCode::NOT_FOUND => "未能获取可用的网页内容（URL 不可用、被拦截或无正文）",
        StatusCode::REQUEST_TIMEOUT => "读取请求体超时",
        StatusCode::PAYLOAD_TOO_LARGE => "请求体超过大小限制",
        StatusCode::TOO_MANY_REQUESTS => "超过 QPS 限制",
        _ if status.is_server_error() => "服务端错误",
        _ => "请求失败",
    };
    if detail.is_empty() {
        format!("HTTP {}: {}", status.as_u16(), hint)
    } else {
        format!("HTTP {}: {} ({})", status.as_u16(), hint, detail)
    }
}

/// 截断过长文本。
fn truncate(text: &str, max_chars: usize) -> String {
    let cleaned = text.trim();
    let mut out = String::new();
    for (idx, ch) in cleaned.chars().enumerate() {
        if idx >= max_chars {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    if out.is_empty() {
        out.push_str("(空响应体)");
    }
    out
}

/// 校验搜索词是否符合官方约束（1～128 个 Unicode 字符）。
pub fn validate_query(query: &str) -> Result<()> {
    let len = query.chars().count();
    if len == 0 || query.trim().is_empty() {
        bail!("搜索词不能为空");
    }
    if len > 128 {
        bail!("搜索词过长：{len} 个字符，最大 128 个字符");
    }
    Ok(())
}

/// 校验 `ori_question` 长度（1～128 个字符）。
pub fn validate_ori_question(value: &str) -> Result<()> {
    let len = value.chars().count();
    if len == 0 {
        bail!("ori_question 不能为空");
    }
    if len > 128 {
        bail!("ori_question 过长：{len} 个字符，最大 128 个字符");
    }
    Ok(())
}

/// 校验抓取 URL：含 host 的 HTTP/HTTPS URL，长度不超过 8192，且不携带用户名或密码。
pub fn validate_fetch_url(url: &str) -> Result<()> {
    let len = url.chars().count();
    if len == 0 {
        bail!("URL 不能为空");
    }
    if len > 8192 {
        bail!("URL 过长：{len} 个字符，最大 8192 个字符");
    }
    let rest = if let Some(r) = url.strip_prefix("https://") {
        r
    } else if let Some(r) = url.strip_prefix("http://") {
        r
    } else {
        bail!("URL 必须以 http:// 或 https:// 开头");
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() {
        bail!("URL 必须包含 host");
    }
    if authority.contains('@') {
        bail!("URL 不能携带用户名或密码");
    }
    Ok(())
}

/// 判断 URL 是否已经是合法可抓取的地址（供调用方前置校验复用）。
#[allow(dead_code)]
pub fn is_valid_fetch_url(url: &str) -> bool {
    validate_fetch_url(url).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_query_accepts_normal() {
        assert!(validate_query("DeepSeek V4").is_ok());
        assert!(validate_query("深度学习").is_ok());
    }

    #[test]
    fn validate_query_rejects_empty_and_long() {
        assert!(validate_query("").is_err());
        assert!(validate_query("   ").is_err());
        assert!(validate_query(&"x".repeat(129)).is_err());
        assert!(validate_query(&"x".repeat(128)).is_ok());
    }

    #[test]
    fn validate_fetch_url_rules() {
        assert!(validate_fetch_url("https://example.com/a").is_ok());
        assert!(validate_fetch_url("http://example.com").is_ok());
        assert!(validate_fetch_url("ftp://example.com").is_err());
        assert!(validate_fetch_url("https://").is_err());
        assert!(validate_fetch_url("https://user:pw@example.com").is_err());
        assert!(validate_fetch_url(&format!("https://example.com/{}", "a".repeat(9000))).is_err());
    }

    #[test]
    fn describe_error_includes_status_and_message() {
        let body = r#"{"error":{"message":"boom","type":"invalid_request_error","param":null,"code":"invalid_request_error"}}"#;
        let text = describe_error(StatusCode::BAD_REQUEST, body);
        assert!(text.contains("400"));
        assert!(text.contains("boom"));
    }

    #[test]
    fn retryable_classification() {
        assert!(is_retryable(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable(StatusCode::REQUEST_TIMEOUT));
        assert!(is_retryable(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(!is_retryable(StatusCode::BAD_REQUEST));
        assert!(!is_retryable(StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn truncate_marks_cut() {
        assert_eq!(truncate("abcdef", 3), "abc…");
        assert_eq!(truncate("  ", 3), "(空响应体)");
    }

    #[test]
    fn client_rejects_empty_key() {
        let err = Client::new(DEFAULT_BASE_URL, "  ", Duration::from_secs(1), 0).unwrap_err();
        assert!(err.to_string().contains("API Key"));
    }

    #[test]
    fn client_trims_base_url() {
        let client = Client::new(
            "https://api.deepseek.com/",
            "sk-test",
            Duration::from_secs(1),
            0,
        )
        .unwrap();
        assert_eq!(client.base_url(), "https://api.deepseek.com");
    }
}
