# dws — DeepSeek 网页搜索与抓取 CLI

**[English](README.md) | [简体中文](README.zh-CN.md)**

对 DeepSeek 官方 [网页搜索与抓取 API](https://api-docs.deepseek.com/zh-cn/guides/search/) 的命令行封装：

| API | Endpoint | 说明 |
| --- | --- | --- |
| Search | `POST https://api.deepseek.com/v1/tools/search` | 按关键词搜索网页，返回结构化结果 |
| Fetch | `POST https://api.deepseek.com/v1/tools/fetch` | 抓取单个网页，返回 Markdown 正文 |

两者共用 DeepSeek 的 Bearer 鉴权。接口字段的完整说明见 [`docs/api.md`](docs/api.md)。

## 特性

- **三种配置来源** —— API Key 可来自 `--api-key`、环境变量 `DEEPSEEK_API_KEY`、`~/.deepseek-websearch-cli/.env` 或配置文件。
- **双语界面** —— 自动跟随系统语言（English / 简体中文），可用 `--locale` 覆盖。
- **跨平台** —— 支持 Windows、macOS、Linux，CI 覆盖三大平台的构建与测试。
- **便于管道处理** —— 支持 `text`、`json`、`markdown` 三种输出格式。
- **稳定可靠** —— 408/429/5xx 走指数退避重试，官方文档中的每个错误码都有友好提示。

## 安装

### 源码安装

```bash
cargo install --path .

# 或本地构建
cargo build --release
./target/release/dws --help
```

### 预编译二进制

从 [Releases 页面](https://github.com/lisniuse/deepseek-websearch-cli/releases) 下载对应平台的压缩包，把 `dws`（Windows 为 `dws.exe`）加入 `PATH` 即可。

### 各平台路径

| 平台 | 配置文件 | `.env` 文件 |
| --- | --- | --- |
| Linux | `$XDG_CONFIG_HOME/deepseek-websearch/config.toml` | `~/.deepseek-websearch-cli/.env` |
| macOS | `~/Library/Application Support/deepseek-websearch/config.toml` | `~/.deepseek-websearch-cli/.env` |
| Windows | `%APPDATA%\deepseek-websearch\config.toml` | `%USERPROFILE%\.deepseek-websearch-cli\.env` |

执行 `dws config path` 可以打印本机实际使用的路径。

> 进阶用法：可通过 `DWS_CONFIG_DIR` / `DWS_ENV_DIR` 改变配置文件与 `.env` 文件的位置（便于测试或非标准部署）。

## 配置

API Key 按以下顺序查找，命中即止：

1. `--api-key <KEY>` 命令行参数
2. 环境变量 `DEEPSEEK_API_KEY`
3. `~/.deepseek-websearch-cli/.env` 文件
4. 配置文件（Windows 为 `%APPDATA%\deepseek-websearch\config.toml`）

### 方式一：环境变量

```bash
export DEEPSEEK_API_KEY="sk-..."          # Linux / macOS
setx DEEPSEEK_API_KEY "sk-..."            # Windows（PowerShell 用 $env:DEEPSEEK_API_KEY = "sk-..."）
```

### 方式二：`.env` 文件

新建 `~/.deepseek-websearch-cli/.env`：

```bash
mkdir -p ~/.deepseek-websearch-cli
cat > ~/.deepseek-websearch-cli/.env <<'EOF'
DEEPSEEK_API_KEY=sk-...
EOF
```

同一个文件里也支持 `DEEPSEEK_BASE_URL`。解析器支持 `KEY=VALUE`、`export KEY=VALUE`、`#` 注释行与带引号的值。

也可以让 CLI 直接帮你写入：

```bash
dws config set-key "sk-..." --env-file
```

### 方式三：配置文件

```bash
dws config set-key "sk-..."
dws config show        # 显示当前配置（Key 会被掩码）
dws config path        # 打印配置文件与 .env 文件的路径
```

## 用法

### 网页搜索

```bash
dws search "DeepSeek V4 模型 发布"
dws search "DeepSeek" --count 20
dws search "DeepSeek site:github.com OR site:huggingface.co lang:zh after:2025-01-01"
dws search "DeepSeek" --content              # 同时抓取并展示网页正文
dws search "DeepSeek" --summary              # 展示 600～800 字的长摘要
dws search "DeepSeek" --links-only           # 只看标题与链接
dws search "DeepSeek" --format json          # 结构化输出，便于管道处理
dws search "DeepSeek" --format markdown      # Markdown 输出，便于粘贴给模型
echo "DeepSeek" | dws search -               # 从标准输入读取搜索词
```

文本输出示例：

```
1. ★ DeepSeek V4 模型发布
   https://example.com/deepseek-v4
   Example News · 2026-06-01T00:00:00+00:00 · 排序 2.4 · 极权威 3
   DeepSeek V4 已上线，支持工具调用与长上下文。
```

`★` 表示 `rerank_score >= 1.5`（官方定义的优质结果）；`极权威` 表示 `authority >= 3`。

### 网页抓取

```bash
dws fetch "https://example.com/article"
dws fetch "https://example.com/article" --format markdown
```

输出为页面标题 + Markdown 正文，可直接管道给其他工具：

```bash
dws fetch "https://example.com/article" | head -50
```

### 搜索语法

`query` 会原样传给搜索服务，支持：

| 操作符 | 示例 | 说明 |
| --- | --- | --- |
| `site:` | `DeepSeek site:github.com` | 限定站点，多个用空格分隔并以 `OR` 连接 |
| `-site:` | `DeepSeek -site:example.com` | 排除站点 |
| `-` | `DeepSeek -广告` | 排除含指定关键词的网页 |
| `-"..."` | `DeepSeek -"release notes"` | 排除含指定短语的网页 |
| `"..."` | `DeepSeek "精确短语"` | 精确匹配短语 |
| `lang:` | `DeepSeek lang:zh,en` | 限定语言，逗号分隔 |
| `after:` | `DeepSeek after:2025-01-01` | 限定发布日期之后 |
| `before:` | `DeepSeek before:2026-06-01` | 限定发布日期之前 |

### 全局参数

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `--api-key <KEY>` | — | API Key（等价于环境变量 `DEEPSEEK_API_KEY`） |
| `--base-url <URL>` | `https://api.deepseek.com` | 接口地址（也可由 `DEEPSEEK_BASE_URL` 指定） |
| `--timeout <SEC>` | `30` | 单次请求超时（秒） |
| `--retries <N>` | `2` | 遇到 408/429/5xx 的重试次数（指数退避，尊重 `Retry-After`） |
| `--format <text\|json\|markdown>` | `text` | 输出格式 |
| `--locale <auto\|en\|zh>` | `auto` | 界面语言，`auto` 表示自动识别系统语言 |
| `--no-color` | — | 关闭彩色输出 |
| `-v, --verbose` | — | 打印调试日志到 stderr |

### 多语言

界面语言跟随操作系统语言（`zh-*` 选择简体中文，其它选择英文），也可以手动覆盖：

```bash
dws --locale zh --help
dws --locale en search "DeepSeek"
```

> 注意：clap 自身产生的提示（例如 `error: invalid value`）目前仍为英文。

## 计费提醒

Search / Fetch 均按**成功请求次数**计费：每 1,000 次成功请求 5 元。空结果、失败、超时不计费；`count` 只影响返回条数，不影响单次请求价格。账号级限速为 Search 100 QPS、Fetch 25 QPS，超限返回 HTTP 429。

因此建议先小批量验证，不要把 `dws search` 放在高频循环里。

## 开发

```bash
cargo test          # 单元测试 + 集成测试（wiremock 模拟接口，不发真实请求）
cargo clippy --all-targets
cargo fmt
```

目录结构：

```
src/
  lib.rs      库入口（暴露各模块，便于集成测试）
  main.rs     二进制入口
  cli.rs      命令行参数、本地化帮助、命令分发
  client.rs   HTTP 客户端：鉴权、重试、错误翻译
  config.rs   配置文件与 .env 文件处理
  i18n.rs     语言识别与中英文案表
  model.rs    请求与响应模型
  output.rs   文本 / JSON / Markdown 渲染
tests/
  cli.rs      命令行行为测试
  http.rs     针对 mock 服务的 HTTP 层测试
```

## 许可

[MIT](LICENSE) © 2026 lisniuse
