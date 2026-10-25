//! 命令行层面的集成测试（直接 spawn 编译好的二进制，不依赖真实网络）。

use std::path::Path;
use std::process::{Command, Output};

/// 构造一个隔离了用户配置的子进程环境。
fn run_isolated(args: &[&str], config_home: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dws"));
    command
        .args(args)
        .env_remove("DEEPSEEK_API_KEY")
        .env_remove("DEEPSEEK_BASE_URL")
        .env("APPDATA", config_home)
        .env("XDG_CONFIG_HOME", config_home);
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
fn search_without_api_key_fails_with_hint() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_isolated(&["search", "hello"], dir.path());
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("API Key"), "标准错误输出: {stderr}");
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
    let output = run_isolated(&["fetch", "ftp://example.com"], dir.path());
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("http"), "标准错误输出: {stderr}");
}

#[test]
fn config_path_prints_a_path() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_isolated(&["config", "path"], dir.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("deepseek-websearch"), "标准输出: {stdout}");
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
