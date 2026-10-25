//! DeepSeek 网页搜索与抓取 CLI 入口。

use deepseek_websearch_cli::cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    enable_ansi();

    // `parse_args` 会先确定界面语言，再解析参数，这样 `--help` 等输出也是本地化的。
    let cli = cli::parse_args();
    cli::run(cli).await
}

/// 在 Windows 控制台上开启 ANSI 转义序列支持（Windows 10 1511 之前的 conhost 默认不支持）。
#[cfg(windows)]
fn enable_ansi() {
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        STD_OUTPUT_HANDLE,
    };

    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle.is_null() {
            return;
        }
        let mut mode: u32 = 0;
        if GetConsoleMode(handle, &mut mode) != 0 {
            SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
}

/// 其它平台默认支持 ANSI 转义序列。
#[cfg(not(windows))]
fn enable_ansi() {}
