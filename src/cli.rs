//! 命令行参数定义与命令分发。

use std::io::Read;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use crate::client::{
    validate_fetch_url, validate_ori_question, validate_query, Client, DEFAULT_BASE_URL,
};
use crate::config;
use crate::model::SearchRequest;
use crate::output::{render_fetch, render_search, Format, RenderOptions};

/// DeepSeek 网页搜索与抓取命令行工具。
#[derive(Debug, Parser)]
#[command(
    name = "dws",
    version,
    about = "DeepSeek 网页搜索与抓取 API 命令行工具",
    long_about = "封装 DeepSeek 官方的网页搜索（/v1/tools/search）与网页抓取（/v1/tools/fetch）接口。\n\
                  API Key 依次从 --api-key、环境变量 DEEPSEEK_API_KEY、配置文件读取。",
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// DeepSeek API Key（默认读取环境变量 DEEPSEEK_API_KEY）
    #[arg(
        long,
        global = true,
        env = "DEEPSEEK_API_KEY",
        hide_env_values = true,
        value_name = "KEY"
    )]
    pub api_key: Option<String>,

    /// API Base URL（默认 https://api.deepseek.com，可用配置文件覆盖）
    #[arg(
        long,
        global = true,
        env = "DEEPSEEK_BASE_URL",
        default_value = "",
        value_name = "URL"
    )]
    pub base_url: String,

    /// 单次请求超时时间（秒）
    #[arg(long, global = true, default_value_t = 30, value_name = "SEC")]
    pub timeout: u64,

    /// 遇到 408/429/5xx 时的重试次数
    #[arg(long, global = true, default_value_t = 2, value_name = "N")]
    pub retries: u32,

    /// 输出格式
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    pub format: Format,

    /// 关闭彩色输出
    #[arg(long, global = true)]
    pub no_color: bool,

    /// 打印调试日志
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

/// 子命令。
#[derive(Debug, Subcommand)]
pub enum Command {
    /// 网页搜索：按关键词搜索网页
    Search(SearchArgs),
    /// 网页抓取：抓取单个网页并返回 Markdown 正文
    Fetch(FetchArgs),
    /// 配置管理
    #[command(subcommand)]
    Config(ConfigCommand),
}

/// `dws search` 的参数。
#[derive(Debug, clap::Args)]
pub struct SearchArgs {
    /// 搜索词，支持 site: / -site: / "精确短语" / lang: / after: / before: 等语法
    #[arg(value_name = "QUERY")]
    pub query: String,

    /// 返回条数（1～50）
    #[arg(short = 'n', long, default_value_t = 10, value_parser = clap::value_parser!(u8).range(1..=50))]
    pub count: u8,

    /// 请求网页正文，并在文本输出中一并展示
    #[arg(long)]
    pub content: bool,

    /// 触发搜索的原始问题（1～128 字符），有助于理解搜索意图
    #[arg(long, value_name = "TEXT")]
    pub ori_question: Option<String>,

    /// 用于优先返回本地化搜索结果的 IP 地址
    #[arg(long, value_name = "IP")]
    pub localization: Option<String>,

    /// 只显示标题与链接
    #[arg(short = 'l', long)]
    pub links_only: bool,

    /// 显示长摘要（600～800 字符）而不是短摘要
    #[arg(short = 's', long)]
    pub summary: bool,
}

/// `dws fetch` 的参数。
#[derive(Debug, clap::Args)]
pub struct FetchArgs {
    /// 要抓取的网页地址（HTTP/HTTPS）
    #[arg(value_name = "URL")]
    pub url: String,
}

/// `dws config` 的子命令。
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// 保存 API Key 到本地配置文件
    SetKey {
        /// 要保存的 API Key
        #[arg(value_name = "KEY")]
        key: String,
    },
    /// 显示当前配置（API Key 会被掩码）
    Show,
    /// 显示配置文件路径
    Path,
}

/// 入口：解析参数并执行对应命令。
pub async fn run(cli: Cli) -> Result<()> {
    init_tracing(cli.verbose);
    if cli.no_color {
        colored::control::set_override(false);
    }

    match &cli.command {
        Command::Search(args) => run_search(&cli, args).await,
        Command::Fetch(args) => run_fetch(&cli, args).await,
        Command::Config(cmd) => run_config(cmd),
    }
}

async fn run_search(cli: &Cli, args: &SearchArgs) -> Result<()> {
    let query = resolve_query(&args.query)?;
    validate_query(&query)?;

    if let Some(ori) = &args.ori_question {
        validate_ori_question(ori)?;
    }

    let api_key = resolve_api_key(cli)?;
    let client = build_client(cli, api_key)?;

    let mut request = SearchRequest::new(query);
    request.count = Some(args.count);
    request.ori_question = args.ori_question.clone();
    request.localization = args.localization.clone();
    request.need_content = Some(args.content);

    tracing::debug!(?request, base_url = client.base_url(), "发送搜索请求");
    let response = client.search(&request).await?;

    let opts = RenderOptions {
        links_only: args.links_only,
        summary: args.summary,
        content: args.content,
    };
    println!("{}", render_search(&response, cli.format, &opts));
    Ok(())
}

async fn run_fetch(cli: &Cli, args: &FetchArgs) -> Result<()> {
    validate_fetch_url(&args.url)?;
    let api_key = resolve_api_key(cli)?;
    let client = build_client(cli, api_key)?;

    tracing::debug!(url = %args.url, base_url = client.base_url(), "发送抓取请求");
    let response = client.fetch(&args.url).await?;
    println!("{}", render_fetch(&response, cli.format));
    Ok(())
}

fn run_config(cmd: &ConfigCommand) -> Result<()> {
    match cmd {
        ConfigCommand::SetKey { key } => {
            let key = key.trim().to_string();
            if key.is_empty() {
                bail!("API Key 不能为空");
            }
            let mut cfg = config::load();
            cfg.api_key = Some(key);
            let path = config::save(&cfg)?;
            println!("API Key 已保存到 {}", path.display());
        }
        ConfigCommand::Show => {
            let cfg = config::load();
            println!("配置文件: {}", config::config_path().display());
            match cfg.api_key.as_deref() {
                Some(key) => println!("api_key:  {}", config::mask_key(key)),
                None => println!("api_key:  (未设置)"),
            }
            println!("base_url: {}", resolve_base_url(cfg.base_url.as_deref()));
            if std::env::var("DEEPSEEK_API_KEY").is_ok() {
                println!("环境变量 DEEPSEEK_API_KEY: 已设置（优先于配置文件）");
            }
        }
        ConfigCommand::Path => {
            println!("{}", config::config_path().display());
        }
    }
    Ok(())
}

/// 构造 HTTP 客户端。
fn build_client(cli: &Cli, api_key: String) -> Result<Client> {
    let base_url = resolve_base_url(Some(cli.base_url.as_str()));
    Client::new(
        base_url,
        &api_key,
        Duration::from_secs(cli.timeout),
        cli.retries,
    )
}

/// 解析 API Key：命令行/环境变量 > 配置文件。
fn resolve_api_key(cli: &Cli) -> Result<String> {
    if let Some(key) = cli
        .api_key
        .as_ref()
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
    {
        return Ok(key.to_string());
    }
    let cfg = config::load();
    if let Some(key) = cfg
        .api_key
        .as_ref()
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
    {
        return Ok(key.to_string());
    }
    bail!(
        "未找到 API Key：请设置环境变量 DEEPSEEK_API_KEY，\
         或使用 --api-key，或运行 `dws config set-key <KEY>`"
    )
}

/// 解析 Base URL：命令行 > 配置文件 > 默认值。
fn resolve_base_url(cli_value: Option<&str>) -> String {
    if let Some(value) = cli_value.map(str::trim).filter(|v| !v.is_empty()) {
        return value.trim_end_matches('/').to_string();
    }
    let cfg = config::load();
    cfg.base_url
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.trim_end_matches('/').to_string())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
}

/// 支持用 `-` 从标准输入读取搜索词。
fn resolve_query(query: &str) -> Result<String> {
    if query != "-" {
        return Ok(query.to_string());
    }
    let mut buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut buffer)
        .context("读取标准输入失败")?;
    Ok(buffer.trim().to_string())
}

/// 初始化日志。
fn init_tracing(verbose: bool) {
    use tracing_subscriber::EnvFilter;
    let filter = if verbose {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("dws=debug,info"))
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parse_search_args() {
        let cli = Cli::try_parse_from([
            "dws",
            "search",
            "DeepSeek",
            "--count",
            "5",
            "--content",
            "--summary",
        ])
        .unwrap();
        match cli.command {
            Command::Search(args) => {
                assert_eq!(args.query, "DeepSeek");
                assert_eq!(args.count, 5);
                assert!(args.content);
                assert!(args.summary);
            }
            _ => panic!("expected search command"),
        }
    }

    #[test]
    fn count_out_of_range_rejected() {
        assert!(Cli::try_parse_from(["dws", "search", "q", "--count", "0"]).is_err());
        assert!(Cli::try_parse_from(["dws", "search", "q", "--count", "51"]).is_err());
    }

    #[test]
    fn parse_fetch_args() {
        let cli = Cli::try_parse_from(["dws", "fetch", "https://example.com"]).unwrap();
        match cli.command {
            Command::Fetch(args) => assert_eq!(args.url, "https://example.com"),
            _ => panic!("expected fetch command"),
        }
    }

    #[test]
    fn parse_config_subcommands() {
        let cli = Cli::try_parse_from(["dws", "config", "show"]).unwrap();
        assert!(matches!(cli.command, Command::Config(ConfigCommand::Show)));
        let cli = Cli::try_parse_from(["dws", "config", "path"]).unwrap();
        assert!(matches!(cli.command, Command::Config(ConfigCommand::Path)));
        let cli = Cli::try_parse_from(["dws", "config", "set-key", "sk-x"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Config(ConfigCommand::SetKey { .. })
        ));
    }

    #[test]
    fn global_flags_apply_to_subcommands() {
        let cli =
            Cli::try_parse_from(["dws", "search", "q", "--format", "json", "--no-color"]).unwrap();
        assert_eq!(cli.format, Format::Json);
    }

    #[test]
    fn base_url_resolution_prefers_cli() {
        let cli =
            Cli::try_parse_from(["dws", "search", "q", "--base-url", "https://x.dev/"]).unwrap();
        assert_eq!(
            resolve_base_url(Some(cli.base_url.as_str())),
            "https://x.dev"
        );
    }
}
