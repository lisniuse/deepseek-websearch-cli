//! 结果渲染：文本 / JSON / Markdown 三种输出格式。

use clap::ValueEnum;
use colored::Colorize;

use crate::model::{FetchResponse, SearchResponse, SearchResult};

/// 输出格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// 人类可读的终端文本。
    Text,
    /// 原始 JSON（结构化，便于管道处理）。
    Json,
    /// Markdown（适合粘贴到笔记或交给模型）。
    Markdown,
}

/// 文本渲染的开关。
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderOptions {
    /// 只显示标题与链接。
    pub links_only: bool,
    /// 显示长摘要而不是短摘要。
    pub summary: bool,
    /// 显示网页正文（需要请求时同时带上 `need_content`）。
    pub content: bool,
}

/// 渲染搜索结果。
pub fn render_search(response: &SearchResponse, format: Format, opts: &RenderOptions) -> String {
    match format {
        Format::Json => serde_json::to_string_pretty(response)
            .unwrap_or_else(|err| format!("{{\"error\":\"序列化失败: {err}\"}}")),
        Format::Text => render_search_text(response, opts),
        Format::Markdown => render_search_markdown(response, opts),
    }
}

/// 渲染抓取结果。
pub fn render_fetch(response: &FetchResponse, format: Format) -> String {
    match format {
        Format::Json => serde_json::to_string_pretty(response)
            .unwrap_or_else(|err| format!("{{\"error\":\"序列化失败: {err}\"}}")),
        Format::Text => {
            let mut out = String::new();
            if !response.title.is_empty() {
                out.push_str(&response.title.bold().to_string());
                out.push_str("\n\n");
            }
            out.push_str(&response.content);
            if !response.content.ends_with('\n') {
                out.push('\n');
            }
            out
        }
        Format::Markdown => {
            let mut out = String::new();
            if !response.title.is_empty() {
                out.push_str(&format!("# {}\n\n", response.title));
            }
            out.push_str(&response.content);
            if !response.content.ends_with('\n') {
                out.push('\n');
            }
            out
        }
    }
}

fn render_search_text(response: &SearchResponse, opts: &RenderOptions) -> String {
    if response.results.is_empty() {
        return "未找到匹配结果。".to_string();
    }
    let mut out = String::new();
    for (idx, item) in response.results.iter().enumerate() {
        let number = format!("{}.", idx + 1).bold().to_string();
        if opts.links_only {
            out.push_str(&format!(
                "{number} {} {}\n",
                item.title.bold(),
                item.url.bright_blue()
            ));
            continue;
        }
        out.push_str(&format!(
            "{number} {}{}\n",
            if item.is_high_quality() { "★ " } else { "" },
            item.title.bold()
        ));
        out.push_str(&format!("   {}\n", item.url.bright_blue()));
        let meta = meta_line(item);
        if !meta.is_empty() {
            out.push_str(&format!("   {}\n", meta.dimmed()));
        }
        let body = if opts.summary {
            &item.summary
        } else {
            &item.snippet
        };
        if !body.is_empty() {
            out.push_str(&format!("   {body}\n"));
        }
        if opts.content && !item.content.is_empty() {
            out.push_str(&format!("{}\n", indent_block(&item.content, "   ")));
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn render_search_markdown(response: &SearchResponse, opts: &RenderOptions) -> String {
    if response.results.is_empty() {
        return "未找到匹配结果。".to_string();
    }
    let mut out = String::new();
    for (idx, item) in response.results.iter().enumerate() {
        out.push_str(&format!("{}. [{}]({})\n", idx + 1, item.title, item.url));
        let meta = meta_line(item);
        if !meta.is_empty() {
            out.push_str(&format!("   - {meta}\n"));
        }
        let body = if opts.summary {
            &item.summary
        } else {
            &item.snippet
        };
        if !body.is_empty() {
            out.push_str(&format!("\n   > {body}\n"));
        }
        if opts.content && !item.content.is_empty() {
            out.push('\n');
            out.push_str(&indent_block(&item.content, "   "));
            out.push('\n');
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// 拼接站点 / 日期 / 排序分 / 权威度等元信息。
fn meta_line(item: &SearchResult) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(site) = item.site_name.as_ref().filter(|s| !s.is_empty()) {
        parts.push(site.clone());
    }
    if let Some(date) = item.date.as_ref().filter(|s| !s.is_empty()) {
        parts.push(date.clone());
    }
    parts.push(format!("排序 {}", item.rerank_score));
    if item.authority >= 0 {
        parts.push(if item.is_authoritative() {
            format!("极权威 {}", item.authority)
        } else {
            format!("权威 {}", item.authority)
        });
    }
    parts.join(" · ")
}

/// 给多行文本统一加缩进。
fn indent_block(text: &str, indent: &str) -> String {
    text.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{indent}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SearchResponse {
        SearchResponse {
            results: vec![SearchResult {
                rerank_score: 2.4,
                title: "标题".into(),
                url: "https://example.com/a".into(),
                snippet: "短摘要".into(),
                summary: "长摘要".into(),
                content: "正文第一行\n正文第二行".into(),
                site_name: Some("Example".into()),
                site_icon: None,
                date: Some("2026-06-01T00:00:00+00:00".into()),
                authority: 3,
            }],
        }
    }

    #[test]
    fn text_output_contains_key_fields() {
        let text = render_search(&sample(), Format::Text, &RenderOptions::default());
        assert!(text.contains("标题"));
        assert!(text.contains("https://example.com/a"));
        assert!(text.contains("短摘要"));
        assert!(text.contains("Example"));
        assert!(text.contains("排序 2.4"));
        assert!(text.contains("权威 3"));
    }

    #[test]
    fn links_only_hides_snippet() {
        let opts = RenderOptions {
            links_only: true,
            ..Default::default()
        };
        let text = render_search(&sample(), Format::Text, &opts);
        assert!(text.contains("https://example.com/a"));
        assert!(!text.contains("短摘要"));
    }

    #[test]
    fn summary_option_switches_body() {
        let opts = RenderOptions {
            summary: true,
            ..Default::default()
        };
        let text = render_search(&sample(), Format::Text, &opts);
        assert!(text.contains("长摘要"));
        assert!(!text.contains("短摘要"));
    }

    #[test]
    fn content_option_appends_body() {
        let opts = RenderOptions {
            content: true,
            ..Default::default()
        };
        let text = render_search(&sample(), Format::Text, &opts);
        assert!(text.contains("正文第一行"));
    }

    #[test]
    fn empty_results_message() {
        let empty = SearchResponse { results: vec![] };
        let text = render_search(&empty, Format::Text, &RenderOptions::default());
        assert!(text.contains("未找到匹配结果"));
    }

    #[test]
    fn json_output_is_valid_json() {
        let json = render_search(&sample(), Format::Json, &RenderOptions::default());
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["results"][0]["title"], "标题");
    }

    #[test]
    fn markdown_output_has_link() {
        let md = render_search(&sample(), Format::Markdown, &RenderOptions::default());
        assert!(md.contains("[标题](https://example.com/a)"));
    }

    #[test]
    fn fetch_text_output_includes_title_and_body() {
        let res = FetchResponse {
            title: "文章".into(),
            content: "# 正文".into(),
        };
        let text = render_fetch(&res, Format::Text);
        assert!(text.contains("文章"));
        assert!(text.contains("# 正文"));
    }
}
