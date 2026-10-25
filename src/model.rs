//! DeepSeek 网页搜索 / 抓取 API 的请求与响应模型。
//!
//! 参考官方文档：
//! - 网页搜索 API Reference: POST /v1/tools/search
//! - 网页抓取 API Reference: POST /v1/tools/fetch

use serde::{Deserialize, Serialize};

/// `POST /v1/tools/search` 的请求体。
///
/// 服务端会忽略请求体中的未知字段，因此所有可选字段在未设置时直接省略。
#[derive(Debug, Clone, Serialize)]
pub struct SearchRequest {
    /// 搜索词，1～128 个 Unicode 字符。
    pub query: String,
    /// 返回条数，1～50，默认 10。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u8>,
    /// 触发搜索的原始问题，1～128 个字符。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ori_question: Option<String>,
    /// 用于优先返回本地化搜索结果的 IP 地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub localization: Option<String>,
    /// 是否在 `content` 字段返回网页正文。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub need_content: Option<bool>,
}

impl SearchRequest {
    /// 构造一个只带搜索词的请求体。
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            count: None,
            ori_question: None,
            localization: None,
            need_content: None,
        }
    }
}

/// `POST /v1/tools/search` 的响应体。
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SearchResponse {
    /// 搜索结果；没有匹配结果时为空数组。
    #[serde(default)]
    pub results: Vec<SearchResult>,
}

/// 单条搜索结果。
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SearchResult {
    /// 排序分数，0～3；>= 1.5 视为优质结果。
    #[serde(default)]
    pub rerank_score: f64,
    /// 标题。
    #[serde(default)]
    pub title: String,
    /// 结果链接。
    #[serde(default)]
    pub url: String,
    /// 短摘要，通常 100～200 字符。
    #[serde(default)]
    pub snippet: String,
    /// 长摘要，通常 600～800 字符，适合喂给模型。
    #[serde(default)]
    pub summary: String,
    /// 网页正文；未请求或缺失时为空字符串。
    #[serde(default)]
    pub content: String,
    /// 站点名称，缺失时为 null。
    pub site_name: Option<String>,
    /// 站点图标 URL，缺失时为 null。
    pub site_icon: Option<String>,
    /// 发布时间，UTC RFC 3339，未知时为 null。
    pub date: Option<String>,
    /// 网页权威性 0～4，-1 表示未知。
    #[serde(default)]
    pub authority: i32,
}

impl SearchResult {
    /// `rerank_score >= 1.5` 为优质结果。
    pub fn is_high_quality(&self) -> bool {
        self.rerank_score >= 1.5
    }

    /// `authority >= 3` 为极权威结果。
    pub fn is_authoritative(&self) -> bool {
        self.authority >= 3
    }
}

/// `POST /v1/tools/fetch` 的请求体。
#[derive(Debug, Clone, Serialize)]
pub struct FetchRequest {
    /// 要抓取的网页地址，长度不超过 8192。
    pub url: String,
}

/// `POST /v1/tools/fetch` 的响应体。
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct FetchResponse {
    /// 页面标题；缺失时为空字符串。
    #[serde(default)]
    pub title: String,
    /// 抓取后的 Markdown 正文。
    #[serde(default)]
    pub content: String,
}

/// OpenAI 兼容的错误响应体。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ApiErrorEnvelope {
    #[serde(default)]
    pub error: ApiErrorBody,
}

/// 错误详情。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ApiErrorBody {
    pub message: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// 官方错误格式中该字段恒为 null，保留以兼容原始报文。
    #[allow(dead_code)]
    pub param: Option<serde_json::Value>,
    pub code: Option<String>,
}

impl ApiErrorBody {
    /// 拼接出便于阅读的错误描述。
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(code) = &self.code {
            parts.push(code.clone());
        } else if let Some(kind) = &self.kind {
            parts.push(kind.clone());
        }
        if let Some(message) = &self.message {
            parts.push(message.clone());
        }
        if parts.is_empty() {
            "未知错误".to_string()
        } else {
            parts.join(": ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_request_omits_unset_optional_fields() {
        let req = SearchRequest::new("hello");
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"query":"hello"}"#);
    }

    #[test]
    fn search_request_serializes_all_fields() {
        let req = SearchRequest {
            query: "q".into(),
            count: Some(5),
            ori_question: Some("o".into()),
            localization: Some("1.2.3.4".into()),
            need_content: Some(true),
        };
        let json: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(json["count"], 5);
        assert_eq!(json["need_content"], true);
        assert_eq!(json["localization"], "1.2.3.4");
    }

    #[test]
    fn deserializes_search_response() {
        let raw = r#"{
          "results": [
            {
              "rerank_score": 2.4,
              "title": "t",
              "url": "https://example.com",
              "snippet": "s",
              "summary": "sum",
              "content": "c",
              "site_name": "Example News",
              "site_icon": "https://example.com/favicon.ico",
              "date": "2026-06-01T00:00:00+00:00",
              "authority": 3
            }
          ]
        }"#;
        let res: SearchResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(res.results.len(), 1);
        let r = &res.results[0];
        assert!((r.rerank_score - 2.4).abs() < f64::EPSILON);
        assert!(r.is_high_quality());
        assert!(r.is_authoritative());
        assert_eq!(r.date.as_deref(), Some("2026-06-01T00:00:00+00:00"));
    }

    #[test]
    fn deserializes_empty_results() {
        let res: SearchResponse = serde_json::from_str(r#"{"results": []}"#).unwrap();
        assert!(res.results.is_empty());
    }

    #[test]
    fn error_body_describe() {
        let env: ApiErrorEnvelope = serde_json::from_str(
            r#"{"error":{"message":"'count' must be between 1 and 50","type":"invalid_request_error","param":null,"code":"invalid_request_error"}}"#,
        )
        .unwrap();
        assert!(env
            .error
            .describe()
            .contains("'count' must be between 1 and 50"));
        assert!(env.error.describe().contains("invalid_request_error"));
    }
}
