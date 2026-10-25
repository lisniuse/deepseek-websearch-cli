//! 针对 HTTP 层的集成测试：使用 wiremock 模拟 DeepSeek 接口。

use std::time::Duration;

use deepseek_websearch_cli::client::Client;
use deepseek_websearch_cli::model::SearchRequest;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client_for(server: &MockServer, retries: u32) -> Client {
    Client::new(server.uri(), "sk-test-key", Duration::from_secs(5), retries)
        .expect("构造客户端失败")
}

#[tokio::test]
async fn search_returns_results() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/search"))
        .and(body_json(serde_json::json!({
            "query": "DeepSeek",
            "count": 5,
            "need_content": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [{
                "rerank_score": 2.4,
                "title": "DeepSeek V4 模型发布",
                "url": "https://example.com/deepseek-v4",
                "snippet": "短摘要",
                "summary": "长摘要",
                "content": "正文",
                "site_name": "Example News",
                "site_icon": null,
                "date": "2026-06-01T00:00:00+00:00",
                "authority": 3
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server, 0);
    let mut request = SearchRequest::new("DeepSeek");
    request.count = Some(5);
    request.need_content = Some(true);

    let response = client.search(&request).await.expect("搜索失败");
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].title, "DeepSeek V4 模型发布");
    assert_eq!(response.results[0].authority, 3);
    assert_eq!(response.results[0].site_icon, None);
}

#[tokio::test]
async fn search_accepts_empty_results() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"results": []})))
        .mount(&server)
        .await;

    let client = client_for(&server, 0);
    let response = client.search(&SearchRequest::new("nothing")).await.unwrap();
    assert!(response.results.is_empty());
}

#[tokio::test]
async fn fetch_returns_markdown() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/fetch"))
        .and(body_json(
            serde_json::json!({"url": "https://example.com/a"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "title": "Example article",
            "content": "# Example article\n\nArticle content."
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server, 0);
    let response = client.fetch("https://example.com/a").await.unwrap();
    assert_eq!(response.title, "Example article");
    assert!(response.content.contains("# Example article"));
}

#[tokio::test]
async fn unauthorized_reports_api_key_problem() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/search"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": {
                "message": "Authentication Fails",
                "type": "authentication_error",
                "param": null,
                "code": "invalid_request_error"
            }
        })))
        .mount(&server)
        .await;

    let client = client_for(&server, 0);
    let err = client
        .search(&SearchRequest::new("q"))
        .await
        .expect_err("应当失败");
    let message = err.to_string();
    assert!(message.contains("401"), "实际错误: {message}");
    assert!(message.contains("API Key"), "实际错误: {message}");
}

#[tokio::test]
async fn rate_limit_is_retried_then_reported() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/search"))
        .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({
            "error": {"message": "rate limited", "type": "rate_limit_error", "param": null, "code": null}
        })))
        .expect(2)
        .mount(&server)
        .await;

    let client = client_for(&server, 1);
    let err = client
        .search(&SearchRequest::new("q"))
        .await
        .expect_err("应当失败");
    let message = err.to_string();
    assert!(message.contains("429"), "实际错误: {message}");
    assert!(message.contains("QPS"), "实际错误: {message}");
}

#[tokio::test]
async fn server_error_is_retried() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/search"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "error": {"message": "internal", "type": "server_error", "param": null, "code": null}
        })))
        .expect(2)
        .mount(&server)
        .await;

    let client = client_for(&server, 1);
    let err = client
        .search(&SearchRequest::new("q"))
        .await
        .expect_err("应当失败");
    assert!(err.to_string().contains("500"), "实际错误: {err}");
}

#[tokio::test]
async fn bad_request_is_not_retried() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/search"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": {
                "message": "'count' must be between 1 and 50",
                "type": "invalid_request_error",
                "param": null,
                "code": "invalid_request_error"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server, 3);
    let err = client
        .search(&SearchRequest::new("q"))
        .await
        .expect_err("应当失败");
    let message = err.to_string();
    assert!(
        message.contains("'count' must be between 1 and 50"),
        "实际错误: {message}"
    );
}

#[tokio::test]
async fn request_carries_bearer_token() {
    use wiremock::matchers::header;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/search"))
        .and(header("authorization", "Bearer sk-test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"results": []})))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server, 0);
    client.search(&SearchRequest::new("q")).await.unwrap();
}
