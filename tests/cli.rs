//! 命令行层面的集成测试（直接 spawn 编译好的二进制，不依赖真实网络）。

use std::path::Path;
use std::process::{Command, Output};

/// 构造一个隔离了用户配置的子进程环境。
///
/// 注意：Windows 上的 `dirs` 走 Known Folder API，读 `APPDATA` / `HOME` 并不能隔离，
/// 因此这里统一用 `DWS_CONFIG_DIR` / `DWS_ENV_DIR` 覆盖配置目录。
fn run_isolated(args: &[&str], home: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dws"));
    command
        .args(args)
        .env_remove("DEEPSEEK_API_KEY")
        .env_remove("DEEPSEEK_BASE_URL")
        .env("DWS_CONFIG_DIR", home.join("config"))
        .env("DWS_ENV_DIR", home.join("env"));
    command.output().expect("无法启动 dws")
}

#[test]
fn help_lists_subcommands() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_isolated(&["--help"], dir.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("search"), "帮助输出: {stdout}");
    assert!(stdout.contains("fetch"), "帮助输出: {stdout}");
    assert!(stdout.contains("config"), "帮助输出: {stdout}");
}

#[test]
fn help_is_localized() {
    let dir = tempfile::tempdir().unwrap();

    let output = run_isolated(&["--locale", "zh", "--help"], dir.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("命令："), "中文帮助输出: {stdout}");

    let output = run_isolated(&["--locale", "en", "--help"], dir.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Commands:"), "英文帮助输出: {stdout}");
}

#[test]
fn search_without_api_key_fails_with_hint() {
    let dir = tempfile::tempdir().unwrap();
    // 固定语言，避免断言依赖运行机器的系统语言
    let output = run_isolated(&["--locale", "en", "search", "hello"], dir.path());
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected = deepseek_websearch_cli::i18n::t("err.no_api_key");
    assert!(
        stderr.contains(expected),
        "标准错误输出: {stderr}\n期望包含: {expected}"
    );
}

#[test]
fn count_out_of_range_is_rejected_before_request() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_isolated(&["search", "hello", "--count", "99"], dir.path());
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("51") || stderr.contains("count"),
        "标准错误输出: {stderr}"
    );
}

#[test]
fn fetch_rejects_non_http_url() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_isolated(
        &["--locale", "en", "fetch", "ftp://example.com"],
        dir.path(),
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(deepseek_websearch_cli::i18n::t("err.url_scheme")),
        "标准错误输出: {stderr}"
    );
}

#[test]
fn config_path_prints_both_paths() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_isolated(&["config", "path"], dir.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("config.toml"), "标准输出: {stdout}");
    assert!(stdout.contains(".env"), "标准输出: {stdout}");
}

#[test]
fn config_set_key_then_show_masks_the_key() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_isolated(&["config", "set-key", "sk-abcdefghijklmnop"], dir.path());
    assert!(
        output.status.success(),
        "标准错误输出: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = run_isolated(&["config", "show"], dir.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sk-"), "标准输出: {stdout}");
    assert!(
        !stdout.contains("abcdefghijklmnop"),
        "API Key 未被掩码: {stdout}"
    );
}

#[test]
fn config_set_key_can_write_the_env_file() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_isolated(
        &["config", "set-key", "sk-envfile-1234", "--env-file"],
        dir.path(),
    );
    assert!(
        output.status.success(),
        "标准错误输出: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let env_file = dir.path().join("env").join(".env");
    let content = std::fs::read_to_string(&env_file).expect("应生成 .env 文件");
    assert!(
        content.contains("DEEPSEEK_API_KEY=sk-envfile-1234"),
        ".env 内容: {content}"
    );
}

#[test]
fn api_key_is_read_from_the_env_file() {
    let dir = tempfile::tempdir().unwrap();
    let env_dir = dir.path().join("env");
    std::fs::create_dir_all(&env_dir).unwrap();
    std::fs::write(env_dir.join(".env"), "DEEPSEEK_API_KEY=sk-from-env-file\n").unwrap();

    let output = run_isolated(&["config", "show"], dir.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(".env"), "标准输出: {stdout}");
    assert!(
        stdout.contains("DEEPSEEK_API_KEY"),
        "应提示 .env 中存在 key: {stdout}"
    );
    // .env 中的 key 不应被原样打印
    assert!(
        !stdout.contains("sk-from-env-file"),
        "API Key 泄露了: {stdout}"
    );
}

#[test]
fn locale_can_be_set_in_the_config_file() {
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join("config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("config.toml"), "locale = \"en\"\n").unwrap();

    let output = run_isolated(&["--help"], dir.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Commands:"), "应使用英文帮助: {stdout}");
}

#[test]
fn locale_can_be_set_in_the_env_file() {
    let dir = tempfile::tempdir().unwrap();
    let env_dir = dir.path().join("env");
    std::fs::create_dir_all(&env_dir).unwrap();
    std::fs::write(env_dir.join(".env"), "DWS_LOCALE=zh\n").unwrap();

    let output = run_isolated(&["--help"], dir.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("命令："), "应使用中文帮助: {stdout}");
}

#[test]
fn locale_flag_overrides_the_config_file() {
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join("config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("config.toml"), "locale = \"zh\"\n").unwrap();

    let output = run_isolated(&["--locale", "en", "--help"], dir.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Commands:"), "命令行参数应优先: {stdout}");
}

#[test]
fn config_set_locale_writes_the_config_file() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_isolated(&["config", "set-locale", "en"], dir.path());
    assert!(
        output.status.success(),
        "标准错误输出: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config_file = dir.path().join("config").join("config.toml");
    let content = std::fs::read_to_string(&config_file).expect("应生成配置文件");
    assert!(content.contains("locale = \"en\""), "配置内容: {content}");

    // 写入后生效
    let output = run_isolated(&["--help"], dir.path());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Commands:"), "应使用英文帮助: {stdout}");
}

#[test]
fn config_set_locale_can_write_the_env_file() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_isolated(&["config", "set-locale", "zh", "--env-file"], dir.path());
    assert!(
        output.status.success(),
        "标准错误输出: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let env_file = dir.path().join("env").join(".env");
    let content = std::fs::read_to_string(&env_file).expect("应生成 .env 文件");
    assert!(content.contains("DWS_LOCALE=zh"), ".env 内容: {content}");
}

#[test]
fn config_show_reports_the_effective_locale() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_isolated(&["config", "show"], dir.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("zh-CN") || stdout.contains("en-US"),
        "应显示当前语言: {stdout}"
    );
}
