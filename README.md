# dws — DeepSeek Web Search & Fetch CLI

**[English](README.md) | [简体中文](README.zh-CN.md)**

A command line wrapper for DeepSeek's official [web search & fetch APIs](https://api-docs.deepseek.com/zh-cn/guides/search/):

| API | Endpoint | Description |
| --- | --- | --- |
| Search | `POST https://api.deepseek.com/v1/tools/search` | Search the web by query, returns structured results |
| Fetch | `POST https://api.deepseek.com/v1/tools/fetch` | Fetch a single web page, returns its Markdown body |

Both use the same DeepSeek Bearer authentication. The full field reference is in [`docs/api.md`](docs/api.md).

## Features

- **Three config sources** — API key from `--api-key`, the `DEEPSEEK_API_KEY` environment variable, `~/.deepseek-websearch-cli/.env`, or the config file.
- **Bilingual interface** — automatically follows the system language (English / 简体中文), overridable with `--locale`.
- **Cross-platform** — Windows, macOS and Linux, with a CI matrix covering all three.
- **Pipe friendly** — `text`, `json` and `markdown` output formats.
- **Reliable** — exponential backoff on 408/429/5xx, friendly messages for every documented error code.

## Install

### From source

Requires Rust 1.88 or newer.

```bash
cargo install --path .

# or build locally
cargo build --release
./target/release/dws --help
```

### Prebuilt binaries

Download the archive for your platform from the [Releases page](https://github.com/lisniuse/deepseek-websearch-cli/releases) and put `dws` (or `dws.exe`) on your `PATH`.

### Platform notes

| Platform | Config file | `.env` file |
| --- | --- | --- |
| Linux | `$XDG_CONFIG_HOME/deepseek-websearch/config.toml` | `~/.deepseek-websearch-cli/.env` |
| macOS | `~/Library/Application Support/deepseek-websearch/config.toml` | `~/.deepseek-websearch-cli/.env` |
| Windows | `%APPDATA%\deepseek-websearch\config.toml` | `%USERPROFILE%\.deepseek-websearch-cli\.env` |

`dws config path` prints the resolved paths on your machine.

> Advanced: set `DWS_CONFIG_DIR` / `DWS_ENV_DIR` to relocate the config file and the `.env` file (useful for testing or non-standard deployments).

## Configuration

The API key is resolved from the first source that provides one:

1. `--api-key <KEY>` command line flag
2. `DEEPSEEK_API_KEY` environment variable
3. `~/.deepseek-websearch-cli/.env` file
4. The config file (`%APPDATA%\deepseek-websearch\config.toml` on Windows)

### Option A — environment variable

```bash
export DEEPSEEK_API_KEY="sk-..."          # Linux / macOS
setx DEEPSEEK_API_KEY "sk-..."            # Windows (PowerShell: $env:DEEPSEEK_API_KEY = "sk-...")
```

### Option B — `.env` file

Create `~/.deepseek-websearch-cli/.env`:

```bash
mkdir -p ~/.deepseek-websearch-cli
cat > ~/.deepseek-websearch-cli/.env <<'EOF'
DEEPSEEK_API_KEY=sk-...
EOF
```

`DEEPSEEK_BASE_URL` is supported in the same file. The parser understands `KEY=VALUE`, `export KEY=VALUE`, `#` comments and quoted values.

You can also let the CLI write it for you:

```bash
dws config set-key "sk-..." --env-file
```

### Option C — config file

```bash
dws config set-key "sk-..."
dws config show        # the key is masked
dws config path        # print the config file and .env paths
```

The config file holds `api_key`, `base_url` and `locale`:

```toml
api_key = "sk-..."
base_url = "https://api.deepseek.com"
locale = "zh"
```

### Interface language

The interface language is resolved from the first source that provides one:

1. `--locale <auto\|en\|zh>` command line flag
2. `DWS_LOCALE` environment variable
3. `DWS_LOCALE` in the `.env` file
4. `locale` in the config file
5. The system language (`zh-*` selects 简体中文, everything else selects English)

```bash
dws config set-locale zh              # writes locale = "zh" to the config file
dws config set-locale zh --env-file   # writes DWS_LOCALE=zh to the .env file
dws config set-locale auto            # back to following the system language
```

## Usage

### Web search

```bash
dws search "DeepSeek V4 release"
dws search "DeepSeek" --count 20
dws search "DeepSeek site:github.com OR site:huggingface.co lang:zh after:2025-01-01"
dws search "DeepSeek" --content              # also fetch and show the page body
dws search "DeepSeek" --summary              # show the 600-800 char long summary
dws search "DeepSeek" --links-only           # titles and links only
dws search "DeepSeek" --format json          # structured output for pipelines
dws search "DeepSeek" --format markdown      # Markdown, ready to paste into a model
echo "DeepSeek" | dws search -               # read the query from stdin
```

Text output:

```
1. ★ DeepSeek V4 model released
   https://example.com/deepseek-v4
   Example News · 2026-06-01T00:00:00+00:00 · score 2.4 · highly authoritative 3
   DeepSeek V4 is now available, with tool calling and long-context support.
```

`★` marks a high quality result (`rerank_score >= 1.5`), `highly authoritative` marks `authority >= 3`.

### Web fetch

```bash
dws fetch "https://example.com/article"
dws fetch "https://example.com/article" --format markdown
```

Output is the page title plus its Markdown body, ready to pipe:

```bash
dws fetch "https://example.com/article" | head -50
```

### Search syntax

The `query` is passed through to the search service unchanged, so it supports:

| Operator | Example | Description |
| --- | --- | --- |
| `site:` | `DeepSeek site:github.com` | Restrict to a site; several sites are space separated and joined with `OR` |
| `-site:` | `DeepSeek -site:example.com` | Exclude a site |
| `-` | `DeepSeek -ads` | Exclude pages containing a keyword |
| `-"..."` | `DeepSeek -"release notes"` | Exclude pages containing a phrase |
| `"..."` | `DeepSeek "exact phrase"` | Match a phrase exactly |
| `lang:` | `DeepSeek lang:zh,en` | Restrict by language, comma separated |
| `after:` | `DeepSeek after:2025-01-01` | Only pages published after a date |
| `before:` | `DeepSeek before:2026-06-01` | Only pages published before a date |

### Global options

| Option | Default | Description |
| --- | --- | --- |
| `--api-key <KEY>` | — | API key (same as `DEEPSEEK_API_KEY`) |
| `--base-url <URL>` | `https://api.deepseek.com` | API base URL (also read from `DEEPSEEK_BASE_URL`) |
| `--timeout <SEC>` | `30` | Per-request timeout in seconds |
| `--retries <N>` | `2` | Retries on 408/429/5xx, with exponential backoff honouring `Retry-After` |
| `--format <text\|json\|markdown>` | `text` | Output format |
| `--locale <auto\|en\|zh>` | `auto` | Interface language; `auto` follows the system language |
| `--no-color` | — | Disable colored output |
| `-v, --verbose` | — | Print debug logs to stderr |

### Localization

The interface language follows the operating system locale by default and can be overridden per run or permanently (see [Interface language](#interface-language)):

```bash
dws --locale zh --help
dws --locale en search "DeepSeek"
```

> Note: messages produced by clap itself (for example `error: invalid value`) are still in English.

## Billing

Search and Fetch are billed **per successful request**: 5 CNY per 1,000 requests. Empty results, failures and timeouts are not billed, and `count` only affects how many results come back — not the price. The account level limits are 100 QPS for Search and 25 QPS for Fetch; exceeding them returns HTTP 429.

So try a few queries first instead of putting `dws search` in a tight loop.

## Development

```bash
cargo test          # unit tests + integration tests (wiremock, no real requests)
cargo clippy --all-targets
cargo fmt
```

Layout:

```
src/
  lib.rs      library entry point (exposes the modules for integration tests)
  main.rs     binary entry point
  cli.rs      argument definitions, localized help, command dispatch
  client.rs   HTTP client: auth, retries, error translation
  config.rs   config file + .env file handling
  i18n.rs     locale detection and the translation tables
  model.rs    request/response models
  output.rs   text / JSON / Markdown rendering
tests/
  cli.rs      command line behaviour tests
  http.rs     HTTP layer tests against a mock server
```

## License

[MIT](LICENSE) © 2026 lisniuse
