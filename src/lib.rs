//! DeepSeek 网页搜索与抓取 CLI 的库实现。
//!
//! 官方接口（Base URL `https://api.deepseek.com`）：
//! - `POST /v1/tools/search` 网页搜索
//! - `POST /v1/tools/fetch`  网页抓取
//!
//! 文档：<https://api-docs.deepseek.com/zh-cn/guides/search/>

pub mod cli;
pub mod client;
pub mod config;
pub mod i18n;
pub mod model;
pub mod output;
