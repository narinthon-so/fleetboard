//! `$FB_HOME/config.toml`, all keys optional.

use std::path::Path;

use crate::store::{FbError, Result};
use chrono::Duration;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Config {
    pub stale_after: Duration,
    /// Use `gh` for PR / CI evidence when installed.
    pub gh: bool,
    /// Orchestrator adapter: "none" (default) | "paseo" (M2).
    pub adapter: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            stale_after: Duration::hours(2),
            gh: true,
            adapter: "none".to_string(),
        }
    }
}

#[derive(Deserialize)]
struct Raw {
    stale_after: Option<String>,
    gh: Option<bool>,
    adapter: Option<String>,
}

pub fn load(home: &Path) -> Result<Config> {
    let path = home.join("config.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(e.into()),
    };
    parse(&text).map_err(|m| FbError::Usage(format!("{}: {m}", path.display())))
}

fn parse(text: &str) -> std::result::Result<Config, String> {
    let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut cfg = Config::default();
    if let Some(s) = raw.stale_after {
        cfg.stale_after = parse_duration(&s).ok_or_else(|| {
            format!("bad stale_after {s:?} (use a number plus s, m, h or d, e.g. \"2h\")")
        })?;
    }
    if let Some(g) = raw.gh {
        cfg.gh = g;
    }
    if let Some(a) = raw.adapter {
        cfg.adapter = a;
    }
    Ok(cfg)
}

/// "90s", "30m", "2h", "1d".
pub fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    let (num, unit) = s.split_at(s.len().checked_sub(1)?);
    let n: i64 = num.parse().ok().filter(|n| *n >= 0)?;
    match unit {
        "s" => Some(Duration::seconds(n)),
        "m" => Some(Duration::minutes(n)),
        "h" => Some(Duration::hours(n)),
        "d" => Some(Duration::days(n)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("2h"), Some(Duration::hours(2)));
        assert_eq!(parse_duration("30m"), Some(Duration::minutes(30)));
        assert_eq!(parse_duration("1d"), Some(Duration::days(1)));
        assert_eq!(parse_duration("90s"), Some(Duration::seconds(90)));
        for bad in ["", "h", "2", "2x", "-1h", "1.5h"] {
            assert_eq!(parse_duration(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn defaults_and_overrides() {
        let d = parse("").unwrap();
        assert_eq!(
            (d.stale_after, d.gh, d.adapter.as_str()),
            (Duration::hours(2), true, "none")
        );
        let c = parse("stale_after = \"15m\"\ngh = false\nadapter = \"paseo\"\n").unwrap();
        assert_eq!(
            (c.stale_after, c.gh, c.adapter.as_str()),
            (Duration::minutes(15), false, "paseo")
        );
        assert!(parse("stale_after = \"soon\"").is_err());
    }
}
