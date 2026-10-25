//! DeepSeek 网页搜索与抓取 CLI 入口。

use clap::Parser;
use deepseek_websearch_cli::cli::{self, Cli};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli::run(cli).await
}
