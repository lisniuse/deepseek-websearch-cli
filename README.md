# dws — DeepSeek 网页搜索与抓取 CLI

对 [DeepSeek 官方网页搜索与抓取 API](https://api-docs.deepseek.com/zh-cn/guides/search/) 的命令行封装：

| API | Endpoint | 说明 |
| --- | --- | --- |
| Search | `POST https://api.deepseek.com/v1/tools/search` | 按关键词搜索网页，返回结构化结果 |
| Fetch | `POST https://api.deepseek.com/v1/tools/fetch` | 抓取单个网页，返回 Markdown 正文 |

两者共用 DeepSeek 的 Bearer 鉴权。接口字段的完整说明见 [`docs/api.md`](docs/api.md)。

## 安装

```bash
cargo install --path .

# 或本地开发
cargo build --release
./target/release/dws --help
```

## 配置 API Key

按以下顺序查找，先命中者生效：

1. `--api-key <KEY>` 命令行参数
2. 环境变量 `DEEPSEEK_API_KEY`
3. 配置文件（`dws config path` 查看路径，Windows 为 `%APPDATA%\deepseek-websearch\config.toml`）

```bash
export DEEPSEEK_API_KEY="sk-..."       # 方式二
dws config set-key "sk-..."            # 方式三，写入配置文件
dws config show                        # 查看当前配置（Key 会被掩码）
```

## 用法

### 网页搜索

```bash
dws search "DeepSeek V4 模型 发布"
dws search "DeepSeek" --count 20
dws search "DeepSeek site:github.com OR site:huggingface.co lang:zh after:2025-01-01"
dws search "DeepSeek" --content              # 同时抓取并展示网页正文
dws search "DeepSeek" --summary              # 展示 600~800 字的长摘要
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
| `-"..."` | `DeepSeek -"release notes"` | 排除指定短语 |
| `"..."` | `DeepSeek "精确短语"` | 精确匹配短语 |
| `lang:` | `DeepSeek lang:zh,en` | 限定语言，逗号分隔 |
| `after:` | `DeepSeek after:2025-01-01` | 限定发布日期之后 |
| `before:` | `DeepSeek before:2026-06-01` | 限定发布日期之前 |

### 全局参数

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `--api-key <KEY>` | — | API Key（等价于环境变量 `DEEPSEEK_API_KEY`） |
| `--base-url <URL>` | `https://api.deepseek.com` | 接口地址，可用环境变量 `DEEPSEEK_BASE_URL` 覆盖 |
| `--timeout <SEC>` | `30` | 单次请求超时（秒） |
| `--retries <N>` | `2` | 遇到 408/429/5xx 的重试次数（指数退避，尊重 `Retry-After`） |
| `--format <text\|json\|markdown>` | `text` | 输出格式 |
| `--no-color` | — | 关闭彩色输出 |
| `-v, --verbose` | — | 打印调试日志到 stderr |

## 计费提醒

Search / Fetch 均按**成功请求次数**计费：每 1,000 次成功请求 5 元。空结果、失败、超时不计费；`count` 只影响返回条数，不影响单次请求价格。账号级限速为 Search 100 QPS、Fetch 25 QPS，超限返回 HTTP 429。

因此建议先小批量验证，不要把 `dws search` 放在高频循环里。

## 开发

```bash
cargo test          # 单元测试 + 集成测试（集成测试使用 wiremock 模拟接口，不发真实请求）
cargo clippy --all-targets
cargo fmt
```

目录结构：

```
src/
  lib.rs      库入口（暴露各模块，便于集成测试）
  main.rs     二进制入口
  cli.rs      命令行参数定义与命令分发
  client.rs   HTTP 客户端：鉴权、重试、错误解析
  config.rs   API Key / Base URL 的本地配置
  model.rs    请求与响应模型
  output.rs   文本 / JSON / Markdown 渲染
tests/
  cli.rs      命令行行为测试
  http.rs     针对 mock 服务的 HTTP 层测试
```

## 许可

[MIT](LICENSE) © 2026 lisniuse
