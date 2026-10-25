//! 本地配置：API Key 与 Base URL 的持久化。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

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

/// 配置文件所在目录：`$XDG_CONFIG_HOME/deepseek-websearch`（Windows 为 `%APPDATA%\deepseek-websearch`）。
pub fn config_dir() -> PathBuf {
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
            tracing::warn!(path = %path.display(), error = %err, "配置文件解析失败，已忽略");
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
}
