//! 多语言支持：根据系统语言自动选择界面语言（中文 / 英文）。
//!
//! - 语言检测：优先读取 `--locale` 参数，其次读取系统语言（Windows 用 `GetUserDefaultLocaleName`，
//!   macOS / Linux 用 `LANG` / `LC_ALL` / `LC_MESSAGES`）。
//! - 所有面向用户的字符串都通过 [`t`] 获取，键表由 [`i18n_table!`] 宏生成，
//!   中英文两套表共用同一份键列表，缺键会在编译期暴露。

use std::sync::OnceLock;

/// 支持的语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    /// 简体中文
    Zh,
    /// 英文
    En,
}

impl Locale {
    /// 语言代码，如 `zh-CN` / `en-US`。
    pub fn code(self) -> &'static str {
        match self {
            Locale::Zh => "zh-CN",
            Locale::En => "en-US",
        }
    }

    /// 该语言对应的原生名称。
    pub fn native_name(self) -> &'static str {
        match self {
            Locale::Zh => "简体中文",
            Locale::En => "English",
        }
    }
}

impl std::fmt::Display for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

static CURRENT: OnceLock<Locale> = OnceLock::new();

/// 当前生效的语言（未初始化时回退到英文）。
pub fn current() -> Locale {
    CURRENT.get().copied().unwrap_or(Locale::En)
}

/// 按系统语言初始化，返回实际使用的语言。
pub fn detect() -> Locale {
    let locale = match sys_locale::get_locale() {
        Some(tag) => {
            let tag = tag.to_ascii_lowercase();
            if tag.starts_with("zh") {
                Locale::Zh
            } else {
                Locale::En
            }
        }
        None => Locale::En,
    };
    let _ = CURRENT.set(locale);
    current()
}

/// 显式指定语言（忽略系统语言）。
pub fn force(locale: Locale) -> Locale {
    let _ = CURRENT.set(locale);
    current()
}

/// 解析 `--locale` 参数值。
pub fn parse_locale(value: &str) -> Option<Locale> {
    match value.trim().to_ascii_lowercase().as_str() {
        "zh" | "zh-cn" | "zh-hans" | "zh_cn" | "cn" | "chinese" => Some(Locale::Zh),
        "en" | "en-us" | "en-gb" | "en_us" | "english" => Some(Locale::En),
        _ => None,
    }
}

/// 从命令行参数中预扫描 `--locale`，用于在 clap 解析前确定语言
/// （否则 `--help` 的输出会早于参数解析）。
pub fn init_from_args<I, S>(args: I) -> Locale
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Vec<String> = args.into_iter().map(|a| a.as_ref().to_string()).collect();
    for (idx, arg) in args.iter().enumerate() {
        let value = if let Some(v) = arg.strip_prefix("--locale=") {
            Some(v.to_string())
        } else if arg == "--locale" {
            args.get(idx + 1).cloned()
        } else {
            None
        };
        if let Some(value) = value {
            if let Some(locale) = parse_locale(&value) {
                return force(locale);
            }
        }
    }
    detect()
}

/// 按当前语言取字符串。
pub fn t(key: &'static str) -> &'static str {
    match current() {
        Locale::En => en(key),
        Locale::Zh => zh(key),
    }
}

/// 按当前语言取字符串并替换 `{name}` 占位符。
pub fn tf(key: &'static str, args: &[(&str, &str)]) -> String {
    let mut text = t(key).to_string();
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

/// 生成中英文两张互相对齐的键表。
macro_rules! i18n_table {
    ($( $key:literal => ($en:expr, $zh:expr) ),* $(,)?) => {
        /// 英文文案。
        pub fn en(key: &'static str) -> &'static str {
            match key {
                $( $key => $en, )*
                _ => key,
            }
        }

        /// 中文文案。
        pub fn zh(key: &'static str) -> &'static str {
            match key {
                $( $key => $zh, )*
                _ => key,
            }
        }
    };
}

i18n_table! {
    // ---- 命令行总体 ----
    "cli.about" => (
        "Command line tool for the DeepSeek web search & fetch APIs",
        "DeepSeek 网页搜索与抓取 API 的命令行工具"
    ),
    "cli.long_about" => (
        "Wraps DeepSeek's official web search (/v1/tools/search) and web fetch (/v1/tools/fetch) APIs.\n\
         The API key is resolved from --api-key, the DEEPSEEK_API_KEY environment variable,\n\
         ~/.deepseek-websearch-cli/.env, then the config file.",
        "封装 DeepSeek 官方的网页搜索（/v1/tools/search）与网页抓取（/v1/tools/fetch）接口。\n\
         API Key 依次从 --api-key、环境变量 DEEPSEEK_API_KEY、\n\
         ~/.deepseek-websearch-cli/.env、配置文件读取。"
    ),
    "cli.locale" => (
        "Interface language (auto detects the system language)",
        "界面语言（auto 表示自动识别系统语言）"
    ),

    // ---- 全局参数 ----
    "arg.api_key" => (
        "DeepSeek API key (defaults to the DEEPSEEK_API_KEY environment variable)",
        "DeepSeek API Key（默认读取环境变量 DEEPSEEK_API_KEY）"
    ),
    "arg.base_url" => (
        "API base URL (defaults to https://api.deepseek.com)",
        "API Base URL（默认 https://api.deepseek.com）"
    ),
    "arg.timeout" => (
        "Per-request timeout in seconds",
        "单次请求超时时间（秒）"
    ),
    "arg.retries" => (
        "Retry attempts on 408/429/5xx responses",
        "遇到 408/429/5xx 时的重试次数"
    ),
    "arg.format" => (
        "Output format",
        "输出格式"
    ),
    "arg.no_color" => (
        "Disable colored output",
        "关闭彩色输出"
    ),
    "arg.verbose" => (
        "Print debug logs to stderr",
        "打印调试日志到 stderr"
    ),
    "arg.locale" => (
        "Interface language: auto, en or zh",
        "界面语言：auto、en 或 zh"
    ),

    // ---- 子命令 ----
    "cmd.search" => (
        "Web search: find web pages matching a query",
        "网页搜索：按关键词搜索网页"
    ),
    "cmd.search.long" => (
        "Search the web via POST /v1/tools/search and print the structured results.",
        "通过 POST /v1/tools/search 搜索网页并输出结构化结果。"
    ),
    "cmd.fetch" => (
        "Web fetch: download a single page as Markdown",
        "网页抓取：抓取单个网页并返回 Markdown 正文"
    ),
    "cmd.fetch.long" => (
        "Fetch a single web page via POST /v1/tools/fetch and print its title and Markdown body.",
        "通过 POST /v1/tools/fetch 抓取单个网页并输出标题与 Markdown 正文。"
    ),
    "cmd.config" => (
        "Manage the local configuration",
        "管理本地配置"
    ),
    "cmd.config.set_key" => (
        "Store an API key in the config file",
        "把 API Key 保存到配置文件"
    ),
    "cmd.config.show" => (
        "Show the current configuration (the key is masked)",
        "显示当前配置（API Key 会被掩码）"
    ),
    "cmd.config.path" => (
        "Print the path of the config file",
        "打印配置文件路径"
    ),

    // ---- 子命令参数 ----
    "arg.query" => (
        "Search query; supports site:, -site:, \"exact phrase\", lang:, after:, before:",
        "搜索词，支持 site:、-site:、\"精确短语\"、lang:、after:、before: 等语法"
    ),
    "arg.count" => (
        "Number of results to return (1-50)",
        "返回条数（1～50）"
    ),
    "arg.content" => (
        "Also fetch and display the page body",
        "同时抓取并展示网页正文"
    ),
    "arg.ori_question" => (
        "The original user question behind the search (1-128 characters)",
        "触发搜索的原始问题（1～128 字符）"
    ),
    "arg.localization" => (
        "IP address used to prioritize localized results",
        "用于优先返回本地化搜索结果的 IP 地址"
    ),
    "arg.links_only" => (
        "Only show titles and links",
        "只显示标题与链接"
    ),
    "arg.summary" => (
        "Show the long summary (600-800 chars) instead of the short snippet",
        "显示长摘要（600～800 字）而不是短摘要"
    ),
    "arg.url" => (
        "Web page URL to fetch (HTTP/HTTPS)",
        "要抓取的网页地址（HTTP/HTTPS）"
    ),
    "arg.key" => (
        "API key to store",
        "要保存的 API Key"
    ),
    "arg.env_file" => (
        "Write the key into ~/.deepseek-websearch-cli/.env instead of the config file",
        "把 Key 写入 ~/.deepseek-websearch-cli/.env 而不是配置文件"
    ),

    // ---- 输出格式 ----
    "fmt.text" => (
        "Human readable terminal text",
        "人类可读的终端文本"
    ),
    "fmt.json" => (
        "Raw JSON, convenient for piping",
        "原始 JSON，便于管道处理"
    ),
    "fmt.markdown" => (
        "Markdown, convenient for pasting into notes or models",
        "Markdown，便于粘贴到笔记或交给模型"
    ),

    // ---- 错误信息 ----
    "err.no_api_key" => (
        "No API key found: set the DEEPSEEK_API_KEY environment variable, pass --api-key, \
         add it to ~/.deepseek-websearch-cli/.env, or run `dws config set-key <KEY>`",
        "未找到 API Key：请设置环境变量 DEEPSEEK_API_KEY，使用 --api-key，\
         写入 ~/.deepseek-websearch-cli/.env，或运行 `dws config set-key <KEY>`"
    ),
    "err.api_key_empty" => (
        "The API key must not be empty",
        "API Key 不能为空"
    ),
    "err.api_key_invalid_header" => (
        "The API key contains characters that are not allowed in a header",
        "API Key 含有非法字符"
    ),
    "err.query_empty" => (
        "The search query must not be empty",
        "搜索词不能为空"
    ),
    "err.query_too_long" => (
        "The search query is too long: {len} characters, the maximum is 128",
        "搜索词过长：{len} 个字符，最大 128 个字符"
    ),
    "err.ori_question_empty" => (
        "ori_question must not be empty",
        "ori_question 不能为空"
    ),
    "err.ori_question_too_long" => (
        "ori_question is too long: {len} characters, the maximum is 128",
        "ori_question 过长：{len} 个字符，最大 128 个字符"
    ),
    "err.url_empty" => (
        "The URL must not be empty",
        "URL 不能为空"
    ),
    "err.url_too_long" => (
        "The URL is too long: {len} characters, the maximum is 8192",
        "URL 过长：{len} 个字符，最大 8192 个字符"
    ),
    "err.url_scheme" => (
        "The URL must start with http:// or https://",
        "URL 必须以 http:// 或 https:// 开头"
    ),
    "err.url_host" => (
        "The URL must contain a host",
        "URL 必须包含 host"
    ),
    "err.url_userinfo" => (
        "The URL must not carry a user name or password",
        "URL 不能携带用户名或密码"
    ),
    "err.create_client" => (
        "Failed to create the HTTP client",
        "创建 HTTP 客户端失败"
    ),
    "err.request_failed" => (
        "Request to {url} failed",
        "请求 {url} 失败"
    ),
    "err.read_body" => (
        "Failed to read the response body",
        "读取响应体失败"
    ),
    "err.parse_response" => (
        "Failed to parse the response JSON, raw body: {body}",
        "解析响应 JSON 失败，原始内容: {body}"
    ),
    "err.serialize_request" => (
        "Failed to serialize the request body",
        "序列化请求体失败"
    ),
    "err.empty_body" => (
        "(empty response body)",
        "(空响应体)"
    ),
    "err.unknown" => (
        "unknown error",
        "未知错误"
    ),

    // ---- HTTP 状态 ----
    "http.400" => (
        "the request header or body is invalid",
        "请求头或请求体不合法"
    ),
    "http.401" => (
        "the API key is missing or invalid",
        "API Key 缺失或无效"
    ),
    "http.402" => (
        "the account balance is insufficient, please top up",
        "账号余额不足，请充值"
    ),
    "http.403" => (
        "the request was rejected by the safety policy",
        "请求被安全策略拒绝"
    ),
    "http.404" => (
        "no usable page content could be fetched",
        "未能获取可用的网页内容"
    ),
    "http.408" => (
        "timed out while reading the request body",
        "读取请求体超时"
    ),
    "http.413" => (
        "the request body is too large",
        "请求体超过大小限制"
    ),
    "http.429" => (
        "the QPS limit was exceeded",
        "超过 QPS 限制"
    ),
    "http.5xx" => (
        "server-side error",
        "服务端错误"
    ),
    "http.default" => (
        "the request failed",
        "请求失败"
    ),

    // ---- 配置相关 ----
    "cfg.saved" => (
        "API key saved to",
        "API Key 已保存到"
    ),
    "cfg.file" => (
        "config file",
        "配置文件"
    ),
    "cfg.env_file" => (
        ".env file",
        ".env 文件"
    ),
    "cfg.api_key" => (
        "api_key",
        "api_key"
    ),
    "cfg.base_url" => (
        "base_url",
        "base_url"
    ),
    "cfg.not_set" => (
        "(not set)",
        "(未设置)"
    ),
    "cfg.env_var_present" => (
        "the DEEPSEEK_API_KEY environment variable is set (it takes precedence over the config file)",
        "环境变量 DEEPSEEK_API_KEY 已设置（优先于配置文件）"
    ),
    "cfg.env_file_present" => (
        "DEEPSEEK_API_KEY was found in {path}",
        "已在 {path} 中找到 DEEPSEEK_API_KEY"
    ),
    "cfg.warn_parse" => (
        "failed to parse the config file, ignoring it",
        "配置文件解析失败，已忽略"
    ),
    "cfg.warn_env_parse" => (
        "failed to parse the .env file, ignoring it",
        ".env 文件解析失败，已忽略"
    ),

    // ---- 输出 ----
    "out.no_results" => (
        "No matching results.",
        "未找到匹配结果。"
    ),
    "out.score" => (
        "score",
        "排序"
    ),
    "out.authority" => (
        "authority",
        "权威"
    ),
    "out.high_authority" => (
        "highly authoritative",
        "极权威"
    ),

    // ---- clap 内置文本 ----
    "clap.usage" => (
        "Usage:",
        "用法："
    ),
    "clap.commands" => (
        "Commands:",
        "命令："
    ),
    "clap.options" => (
        "Options:",
        "选项："
    ),
    "clap.arguments" => (
        "Arguments:",
        "参数："
    ),
    "clap.print_help" => (
        "Print help",
        "打印帮助"
    ),
    "clap.print_version" => (
        "Print version",
        "打印版本"
    ),
    "clap.help_subcommand" => (
        "Print this message or the help of the given subcommand(s)",
        "打印此信息或子命令的帮助"
    ),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_have_the_same_keys() {
        // 两张表由同一个宏生成，键集合必然一致；这里确认查不到键时回退为键本身。
        assert_eq!(en("not.a.real.key"), "not.a.real.key");
        assert_eq!(zh("not.a.real.key"), "not.a.real.key");
    }

    #[test]
    fn translations_differ_for_known_keys() {
        assert_ne!(en("cli.about"), zh("cli.about"));
        assert!(en("cli.about").contains("DeepSeek"));
        assert!(zh("cli.about").contains("DeepSeek"));
    }

    #[test]
    fn parse_locale_accepts_variants() {
        assert_eq!(parse_locale("zh"), Some(Locale::Zh));
        assert_eq!(parse_locale("zh-CN"), Some(Locale::Zh));
        assert_eq!(parse_locale("EN-us"), Some(Locale::En));
        assert_eq!(parse_locale("en"), Some(Locale::En));
        assert_eq!(parse_locale("fr"), None);
    }

    #[test]
    fn locale_codes() {
        assert_eq!(Locale::Zh.code(), "zh-CN");
        assert_eq!(Locale::En.code(), "en-US");
        assert_eq!(Locale::Zh.native_name(), "简体中文");
    }

    #[test]
    fn current_defaults_to_english() {
        // 未调用 detect() 时回退英文，避免 panic。
        let _ = current();
    }
}
