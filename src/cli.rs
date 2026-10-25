//! 命令行参数定义与命令分发。

use std::collections::BTreeMap;
use std::io::Read;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, FromArgMatches, Subcommand};

use crate::client::{
    validate_fetch_url, validate_ori_question, validate_query, Client, DEFAULT_BASE_URL,
};
use crate::config;
use crate::i18n;
use crate::model::SearchRequest;
use crate::output::{render_fetch, render_search, Format, RenderOptions};

/// DeepSeek 网页搜索与抓取命令行工具。
#[derive(Debug, clap::Parser)]
#[command(
    name = "dws",
    version,
    about = "DeepSeek web search & fetch command line tool",
    long_about = "Wraps DeepSeek's official web search and web fetch APIs.",
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// DeepSeek API key (defaults to the DEEPSEEK_API_KEY environment variable)
    #[arg(
        long,
        global = true,
        env = "DEEPSEEK_API_KEY",
        hide_env_values = true,
        value_name = "KEY"
    )]
    pub api_key: Option<String>,

    /// API base URL (defaults to https://api.deepseek.com)
    #[arg(
        long,
        global = true,
        env = "DEEPSEEK_BASE_URL",
        default_value = "",
        value_name = "URL"
    )]
    pub base_url: String,

    /// Per-request timeout in seconds
    #[arg(long, global = true, default_value_t = 30, value_name = "SEC")]
    pub timeout: u64,

    /// Retry attempts on 408/429/5xx responses
    #[arg(long, global = true, default_value_t = 2, value_name = "N")]
    pub retries: u32,

    /// Output format
    #[arg(long, global = true, value_enum, default_value = "text")]
    pub format: Format,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Interface language: auto, en or zh
    #[arg(long, global = true, value_name = "LANG", default_value = "auto")]
    pub locale: String,

    /// Print debug logs to stderr
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

/// 子命令。
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Web search: find web pages matching a query
    Search(SearchArgs),
    /// Web fetch: download a single page as Markdown
    Fetch(FetchArgs),
    /// Manage the local configuration
    #[command(subcommand)]
    Config(ConfigCommand),
}

/// `dws search` 的参数。
#[derive(Debug, clap::Args)]
pub struct SearchArgs {
    /// Search query
    #[arg(value_name = "QUERY")]
    pub query: String,

    /// Number of results to return (1-50)
    #[arg(
        short = 'n',
        long,
        default_value_t = 10,
        value_parser = clap::value_parser!(u8).range(1..=50)
    )]
    pub count: u8,

    /// Also fetch and display the page body
    #[arg(long)]
    pub content: bool,

    /// The original user question behind the search (1-128 characters)
    #[arg(long, value_name = "TEXT")]
    pub ori_question: Option<String>,

    /// IP address used to prioritize localized results
    #[arg(long, value_name = "IP")]
    pub localization: Option<String>,

    /// Only show titles and links
    #[arg(short = 'l', long)]
    pub links_only: bool,

    /// Show the long summary instead of the short snippet
    #[arg(short = 's', long)]
    pub summary: bool,
}

/// `dws fetch` 的参数。
#[derive(Debug, clap::Args)]
pub struct FetchArgs {
    /// Web page URL to fetch (HTTP/HTTPS)
    #[arg(value_name = "URL")]
    pub url: String,
}

/// `dws config` 的子命令。
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Store an API key in the config file
    SetKey {
        /// API key to store
        #[arg(value_name = "KEY")]
        key: String,
        /// Write the key into ~/.deepseek-websearch-cli/.env instead of the config file
        #[arg(long)]
        env_file: bool,
    },
    /// Show the current configuration (the key is masked)
    Show,
    /// Print the paths of the config file and the .env file
    Path,
}

/// 构造已经本地化帮助信息的 clap 命令。
pub fn build_command() -> clap::Command {
    let mut cmd = Cli::command();
    cmd = cmd
        .about(i18n::t("cli.about"))
        .long_about(i18n::t("cli.long_about"))
        .mut_arg("api_key", |a| a.help(i18n::t("arg.api_key")))
        .mut_arg("base_url", |a| a.help(i18n::t("arg.base_url")))
        .mut_arg("timeout", |a| a.help(i18n::t("arg.timeout")))
        .mut_arg("retries", |a| a.help(i18n::t("arg.retries")))
        .mut_arg("format", |a| a.help(i18n::t("arg.format")))
        .mut_arg("no_color", |a| a.help(i18n::t("arg.no_color")))
        .mut_arg("locale", |a| a.help(i18n::t("arg.locale")))
        .mut_arg("verbose", |a| a.help(i18n::t("arg.verbose")));

    cmd = cmd
        .mut_subcommand("search", |c| {
            c.about(i18n::t("cmd.search"))
                .long_about(i18n::t("cmd.search.long"))
                .mut_arg("query", |a| a.help(i18n::t("arg.query")))
                .mut_arg("count", |a| a.help(i18n::t("arg.count")))
                .mut_arg("content", |a| a.help(i18n::t("arg.content")))
                .mut_arg("ori_question", |a| a.help(i18n::t("arg.ori_question")))
                .mut_arg("localization", |a| a.help(i18n::t("arg.localization")))
                .mut_arg("links_only", |a| a.help(i18n::t("arg.links_only")))
                .mut_arg("summary", |a| a.help(i18n::t("arg.summary")))
        })
        .mut_subcommand("fetch", |c| {
            c.about(i18n::t("cmd.fetch"))
                .long_about(i18n::t("cmd.fetch.long"))
                .mut_arg("url", |a| a.help(i18n::t("arg.url")))
        })
        .mut_subcommand("config", |c| {
            c.about(i18n::t("cmd.config"))
                .mut_subcommand("set-key", |c| {
                    c.about(i18n::t("cmd.config.set_key"))
                        .mut_arg("key", |a| a.help(i18n::t("arg.key")))
                        .mut_arg("env_file", |a| a.help(i18n::t("arg.env_file")))
                })
                .mut_subcommand("show", |c| c.about(i18n::t("cmd.config.show")))
                .mut_subcommand("path", |c| c.about(i18n::t("cmd.config.path")))
        });

    cmd
}

/// 解析命令行参数（使用本地化后的命令定义）。
pub fn parse_args() -> Cli {
    match build_command().try_get_matches() {
        Ok(matches) => match Cli::from_arg_matches(&matches) {
            Ok(cli) => cli,
            Err(err) => err.exit(),
        },
        Err(err) => {
            use clap::error::ErrorKind;
            match err.kind() {
                ErrorKind::DisplayHelp => {
                    print!("{}", localize_clap_text(&err.to_string()));
                    std::process::exit(0);
                }
                ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
                    print!("{}", localize_clap_text(&err.to_string()));
                    std::process::exit(2);
                }
                _ => err.exit(),
            }
        }
    }
}

/// 把 clap 内置的英文标题替换成当前语言（英文环境下原样返回）。
fn localize_clap_text(text: &str) -> String {
    if i18n::current() == i18n::Locale::En {
        return text.to_string();
    }
    let mut out = text.to_string();
    for (from, key) in [
        ("Usage:", "clap.usage"),
        ("Commands:", "clap.commands"),
        ("Options:", "clap.options"),
        ("Arguments:", "clap.arguments"),
        (
            "Print this message or the help of the given subcommand(s)",
            "clap.help_subcommand",
        ),
        ("Print help", "clap.print_help"),
        ("Print version", "clap.print_version"),
    ] {
        out = out.replace(from, i18n::t(key));
    }
    out
}

/// 入口：解析参数并执行对应命令。
pub async fn run(cli: Cli) -> Result<()> {
    init_tracing(cli.verbose);
    if cli.no_color {
        colored::control::set_override(false);
    }
    // `--locale` 已在参数解析前生效，这里再确认一次，保持语义一致。
    if let Some(locale) = i18n::parse_locale(&cli.locale) {
        i18n::force(locale);
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

    tracing::debug!(?request, base_url = client.base_url(), "search request");
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

    tracing::debug!(url = %args.url, base_url = client.base_url(), "fetch request");
    let response = client.fetch(&args.url).await?;
    println!("{}", render_fetch(&response, cli.format));
    Ok(())
}

fn run_config(cmd: &ConfigCommand) -> Result<()> {
    match cmd {
        ConfigCommand::SetKey { key, env_file } => {
            let key = key.trim().to_string();
            if key.is_empty() {
                bail!("{}", i18n::t("err.api_key_empty"));
            }
            let path = if *env_file {
                let mut updates = BTreeMap::new();
                updates.insert("DEEPSEEK_API_KEY".to_string(), key);
                config::update_env_file(&config::env_file_path(), &updates)?
            } else {
                let mut cfg = config::load();
                cfg.api_key = Some(key);
                config::save(&cfg)?
            };
            println!("{} {}", i18n::t("cfg.saved"), path.display());
        }
        ConfigCommand::Show => {
            let cfg = config::load();
            let env_file = config::load_env_file();

            println!(
                "{}: {}",
                i18n::t("cfg.file"),
                config::config_path().display()
            );
            match cfg.api_key.as_deref() {
                Some(key) => println!("{}: {}", i18n::t("cfg.api_key"), config::mask_key(key)),
                None => println!("{}: {}", i18n::t("cfg.api_key"), i18n::t("cfg.not_set")),
            }
            println!(
                "{}: {}",
                i18n::t("cfg.base_url"),
                resolve_base_url(None, cfg.base_url.as_deref(), &env_file)
            );

            println!("{}: {}", i18n::t("cfg.env_file"), env_file.path.display());
            match env_file.get("DEEPSEEK_API_KEY") {
                Some(_) => println!(
                    "{}",
                    i18n::tf(
                        "cfg.env_file_present",
                        &[("path", &env_file.path.display().to_string())]
                    )
                ),
                None => println!("{}: {}", i18n::t("cfg.api_key"), i18n::t("cfg.not_set")),
            }
            if std::env::var("DEEPSEEK_API_KEY").is_ok() {
                println!("{}", i18n::t("cfg.env_var_present"));
            }
        }
        ConfigCommand::Path => {
            println!(
                "{}: {}",
                i18n::t("cfg.file"),
                config::config_path().display()
            );
            println!(
                "{}: {}",
                i18n::t("cfg.env_file"),
                config::env_file_path().display()
            );
        }
    }
    Ok(())
}

/// 构造 HTTP 客户端。
fn build_client(cli: &Cli, api_key: String) -> Result<Client> {
    let env_file = config::load_env_file();
    let base_url = resolve_base_url(Some(cli.base_url.as_str()), None, &env_file);
    Client::new(
        base_url,
        &api_key,
        Duration::from_secs(cli.timeout),
        cli.retries,
    )
}

/// 解析 API Key：`--api-key` > 环境变量 > `.env` 文件 > 配置文件。
fn resolve_api_key(cli: &Cli) -> Result<String> {
    if let Some(key) = cli
        .api_key
        .as_ref()
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
    {
        return Ok(key.to_string());
    }
    if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
        let key = key.trim();
        if !key.is_empty() {
            return Ok(key.to_string());
        }
    }
    let env_file = config::load_env_file();
    if let Some(key) = env_file.get("DEEPSEEK_API_KEY") {
        return Ok(key.trim().to_string());
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
    bail!("{}", i18n::t("err.no_api_key"))
}

/// 解析 Base URL：`--base-url` > 环境变量 > `.env` 文件 > 配置文件 > 默认值。
fn resolve_base_url(
    cli_value: Option<&str>,
    cfg_value: Option<&str>,
    env_file: &config::EnvFile,
) -> String {
    if let Some(value) = cli_value.map(str::trim).filter(|v| !v.is_empty()) {
        return value.trim_end_matches('/').to_string();
    }
    if let Ok(value) = std::env::var("DEEPSEEK_BASE_URL") {
        let value = value.trim();
        if !value.is_empty() {
            return value.trim_end_matches('/').to_string();
        }
    }
    if let Some(value) = env_file.get("DEEPSEEK_BASE_URL") {
        return value.trim().trim_end_matches('/').to_string();
    }
    if let Some(value) = cfg_value.map(str::trim).filter(|v| !v.is_empty()) {
        return value.trim_end_matches('/').to_string();
    }
    DEFAULT_BASE_URL.to_string()
}

/// 支持用 `-` 从标准输入读取搜索词。
fn resolve_query(query: &str) -> Result<String> {
    if query != "-" {
        return Ok(query.to_string());
    }
    let mut buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut buffer)
        .context("failed to read stdin")?;
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
    use clap::{CommandFactory, Parser, ValueEnum};

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
        build_command().debug_assert();
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
    fn locale_flag_is_accepted() {
        let cli = Cli::try_parse_from(["dws", "search", "q", "--locale", "zh"]).unwrap();
        assert_eq!(cli.locale, "zh");
    }

    #[test]
    fn format_variants_expose_names() {
        assert_eq!(Format::Text.as_str(), "text");
        assert_eq!(Format::Json.as_str(), "json");
        assert_eq!(Format::Markdown.as_str(), "markdown");
        assert_eq!(Format::value_variants().len(), 3);
    }

    #[test]
    fn base_url_resolution_prefers_cli() {
        let env_file = config::EnvFile::default();
        assert_eq!(
            resolve_base_url(Some("https://x.dev/"), None, &env_file),
            "https://x.dev"
        );
        assert_eq!(
            resolve_base_url(Some(""), Some("https://cfg.dev"), &env_file),
            "https://cfg.dev"
        );
        assert_eq!(resolve_base_url(None, None, &env_file), DEFAULT_BASE_URL);
    }

    #[test]
    fn base_url_resolution_reads_env_file() {
        let mut values = BTreeMap::new();
        values.insert(
            "DEEPSEEK_BASE_URL".to_string(),
            "https://env.dev".to_string(),
        );
        let env_file = config::EnvFile {
            path: std::path::PathBuf::new(),
            values,
        };
        assert_eq!(
            resolve_base_url(Some(""), None, &env_file),
            "https://env.dev"
        );
    }
}
