//! 本地配置：API Key 与 Base URL 的持久化。
//!
//! 三种配置来源（优先级由高到低）：
//! 1. 命令行参数 `--api-key` / `--base-url`
//! 2. 环境变量 `DEEPSEEK_API_KEY` / `DEEPSEEK_BASE_URL`
//! 3. `~/.deepseek-websearch-cli/.env` 文件（dotenv 风格）
//! 4. 配置文件（`%APPDATA%\deepseek-websearch\config.toml` 等）

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::i18n;

/// 配置文件内容（TOML）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// DeepSeek API Key。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// API Base URL。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// `.env` 文件内容。
#[derive(Debug, Clone, Default)]
pub struct EnvFile {
    /// 文件路径。
    pub path: PathBuf,
    /// 解析出的键值对。
    pub values: BTreeMap<String, String>,
}

impl EnvFile {
    /// 读取一个键。
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values
            .get(key)
            .map(|v| v.as_str())
            .filter(|v| !v.is_empty())
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// `.env` 文件所在目录：`~/.deepseek-websearch-cli`。
///
/// 可用环境变量 `DWS_ENV_DIR` 覆盖（便于测试或非标准部署）。
pub fn env_file_dir() -> PathBuf {
    if let Some(dir) = non_empty_env("DWS_ENV_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".deepseek-websearch-cli")
}

/// 读取一个非空的环境变量。
fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

/// `.env` 文件路径：`~/.deepseek-websearch-cli/.env`。
pub fn env_file_path() -> PathBuf {
    env_file_dir().join(".env")
}

/// 读取默认位置的 `.env` 文件；文件不存在或损坏时返回空内容，不阻断运行。
pub fn load_env_file() -> EnvFile {
    load_env_file_from(&env_file_path())
}

/// 从指定路径读取 `.env` 文件。
pub fn load_env_file_from(path: &Path) -> EnvFile {
    let Ok(text) = std::fs::read_to_string(path) else {
        return EnvFile {
            path: path.to_path_buf(),
            values: BTreeMap::new(),
        };
    };
    let values = parse_env_text(&text);
    if values.is_empty() && !text.trim().is_empty() {
        tracing::warn!(path = %path.display(), "{}", i18n::t("cfg.warn_env_parse"));
    }
    EnvFile {
        path: path.to_path_buf(),
        values,
    }
}

/// 解析 dotenv 风格的文本。
///
/// 支持 `KEY=VALUE`、`export KEY=VALUE`、`#` 注释行以及单/双引号包裹的值。
pub fn parse_env_text(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        map.insert(key.to_string(), unquote(value.trim()));
    }
    map
}

/// 去掉值两侧的引号。
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            let inner = &value[1..value.len() - 1];
            return if first == b'"' {
                inner.replace("\\n", "\n").replace("\\t", "\t")
            } else {
                inner.to_string()
            };
        }
    }
    value.to_string()
}

/// 配置文件所在目录：`$XDG_CONFIG_HOME/deepseek-websearch`（Windows 为 `%APPDATA%\deepseek-websearch`）。
///
/// 可用环境变量 `DWS_CONFIG_DIR` 覆盖（便于测试或非标准部署）。
pub fn config_dir() -> PathBuf {
    if let Some(dir) = non_empty_env("DWS_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    let base = dirs::config_dir().unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
    });
    base.join("deepseek-websearch")
}

/// 配置文件路径。
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// 读取配置文件；文件不存在或损坏时返回默认配置，不阻断运行。
pub fn load() -> Config {
    load_from(&config_path())
}

/// 从指定路径读取配置文件。
pub fn load_from(path: &Path) -> Config {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Config::default();
    };
    match toml::from_str::<Config>(&text) {
        Ok(cfg) => cfg,
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "{}", i18n::t("cfg.warn_parse"));
            Config::default()
        }
    }
}

/// 保存配置文件（仅写入非空字段）。
pub fn save(cfg: &Config) -> Result<PathBuf> {
    save_to(cfg, &config_path())
}

/// 保存配置到指定路径。
pub fn save_to(cfg: &Config, path: &Path) -> Result<PathBuf> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建配置目录失败: {}", parent.display()))?;
    }
    let text = toml::to_string_pretty(cfg).context("序列化配置失败")?;
    std::fs::write(path, text).with_context(|| format!("写入配置文件失败: {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }

    Ok(path.to_path_buf())
}

/// 掩码显示 API Key，保留首尾少量字符。
pub fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        return "*".repeat(chars.len());
    }
    let head: String = chars.iter().take(3).collect();
    let tail: String = chars.iter().skip(chars.len() - 4).collect();
    format!("{head}{}{tail}", "*".repeat(chars.len() - 7))
}

/// 更新 `.env` 文件中的若干键（不存在则追加），保留其它行原样。
pub fn update_env_file(path: &Path, updates: &BTreeMap<String, String>) -> Result<PathBuf> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create directory failed: {}", parent.display()))?;
    }
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    let mut written: Vec<&str> = Vec::new();

    for line in existing.lines() {
        let trimmed = line.trim();
        let stripped = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let key = stripped
            .split_once('=')
            .map(|(k, _)| k.trim())
            .unwrap_or("");
        if let Some(value) = updates.get(key) {
            if !written.contains(&key) {
                lines.push(format!("{key}={value}"));
                written.push(key);
            }
            continue;
        }
        lines.push(line.to_string());
    }

    for (key, value) in updates {
        if !written.contains(&key.as_str()) {
            lines.push(format!("{key}={value}"));
        }
    }

    let mut content = lines.join("\n");
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    std::fs::write(path, content)
        .with_context(|| format!("write file failed: {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }

    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("config.toml");
        let cfg = Config {
            api_key: Some("sk-test".into()),
            base_url: Some("https://example.com".into()),
        };
        save_to(&cfg, &path).unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded.api_key.as_deref(), Some("sk-test"));
        assert_eq!(loaded.base_url.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn load_missing_file_is_default() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load_from(&dir.path().join("nope.toml"));
        assert!(loaded.api_key.is_none());
    }

    #[test]
    fn load_broken_file_is_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "this is not = = toml").unwrap();
        let loaded = load_from(&path);
        assert!(loaded.api_key.is_none());
    }

    #[test]
    fn mask_key_hides_middle() {
        // 保留前 3 位与后 4 位，中间用 * 遮盖
        assert_eq!(mask_key("sk-1234567890abcdef"), "sk-************cdef");
        assert_eq!(mask_key("short"), "*****");
        assert_eq!(mask_key("12345678"), "********");
    }

    #[test]
    fn env_parsing_handles_common_shapes() {
        let text = r#"
# a comment
DEEPSEEK_API_KEY=sk-plain
export DEEPSEEK_BASE_URL=https://example.com
QUOTED="sk-quoted"
SINGLE='sk-single'
EMPTY=
NO_EQUALS_SIGN
"#;
        let map = parse_env_text(text);
        assert_eq!(map.get("DEEPSEEK_API_KEY").unwrap(), "sk-plain");
        assert_eq!(map.get("DEEPSEEK_BASE_URL").unwrap(), "https://example.com");
        assert_eq!(map.get("QUOTED").unwrap(), "sk-quoted");
        assert_eq!(map.get("SINGLE").unwrap(), "sk-single");
        assert_eq!(map.get("EMPTY").unwrap(), "");
        assert!(!map.contains_key("NO_EQUALS_SIGN"));
        assert!(!map.contains_key("# a comment"));
    }

    #[test]
    fn env_parsing_keeps_equals_in_value() {
        let map = parse_env_text("KEY=a=b=c\n");
        assert_eq!(map.get("KEY").unwrap(), "a=b=c");
    }

    #[test]
    fn env_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".deepseek-websearch-cli").join(".env");
        let mut updates = BTreeMap::new();
        updates.insert("DEEPSEEK_API_KEY".to_string(), "sk-one".to_string());
        update_env_file(&path, &updates).unwrap();

        let loaded = load_env_file_from(&path);
        assert_eq!(loaded.get("DEEPSEEK_API_KEY"), Some("sk-one"));

        // 再次写入应覆盖而不是追加
        let mut updates = BTreeMap::new();
        updates.insert("DEEPSEEK_API_KEY".to_string(), "sk-two".to_string());
        update_env_file(&path, &updates).unwrap();
        let loaded = load_env_file_from(&path);
        assert_eq!(loaded.get("DEEPSEEK_API_KEY"), Some("sk-two"));
        assert_eq!(loaded.values.len(), 1);
    }

    #[test]
    fn env_file_update_preserves_other_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        std::fs::write(&path, "# keep me\nOTHER=1\nDEEPSEEK_API_KEY=old\n").unwrap();
        let mut updates = BTreeMap::new();
        updates.insert("DEEPSEEK_API_KEY".to_string(), "new".to_string());
        update_env_file(&path, &updates).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# keep me"));
        assert!(text.contains("OTHER=1"));
        assert!(text.contains("DEEPSEEK_API_KEY=new"));
        assert!(!text.contains("old"));
    }

    #[test]
    fn missing_env_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load_env_file_from(&dir.path().join(".env"));
        assert!(loaded.is_empty());
        assert_eq!(loaded.get("DEEPSEEK_API_KEY"), None);
    }
}
