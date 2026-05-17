//! Runtime configuration loaded from environment variables.
//!
//! Ported from `internal/config/config.go`. Each env var falls back to its
//! default when unset, empty, or unparseable.

use std::env;
use std::time::Duration;

/// All runtime configuration values.
#[derive(Debug, Clone)]
pub struct Config {
    /// Listen address. May begin with `:` (e.g. `":8080"`) to bind all
    /// interfaces — the caller is responsible for translating that to a
    /// concrete `SocketAddr`.
    pub listen: String,
    /// Maximum permitted response size in bytes.
    pub size_limit: u64,
    /// Buffer size for streamed copies in bytes. Retained for parity with the
    /// Go config schema; reqwest manages its own internal buffering, so the
    /// value is loaded but not currently consumed by the Rust port.
    #[allow(dead_code)]
    pub buffer_size: usize,
    /// Upstream request timeout.
    pub upstream_timeout: Duration,
    /// Graceful shutdown timeout.
    pub shutdown_timeout: Duration,
}

impl Config {
    /// Load configuration from environment variables, falling back to defaults
    /// whenever a value is unset, empty, or fails to parse.
    pub fn from_env() -> Self {
        Self {
            listen: env_or("LISTEN", ":8080"),
            size_limit: env_u64_or("SIZE_LIMIT", 1_072_668_082_176),
            buffer_size: env_u64_or("BUFFER_SIZE", 32 * 1024) as usize,
            upstream_timeout: env_duration_or("UPSTREAM_TIMEOUT", Duration::from_secs(30)),
            shutdown_timeout: env_duration_or("SHUTDOWN_TIMEOUT", Duration::from_secs(10)),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    match env::var(key) {
        Ok(v) if !v.is_empty() => v,
        _ => default.to_string(),
    }
}

fn env_u64_or(key: &str, default: u64) -> u64 {
    match env::var(key) {
        Ok(v) if !v.is_empty() => v.parse::<u64>().unwrap_or(default),
        _ => default,
    }
}

fn env_duration_or(key: &str, default: Duration) -> Duration {
    match env::var(key) {
        Ok(v) if !v.is_empty() => humantime::parse_duration(&v).unwrap_or(default),
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env mutations are process-global; serialize tests to avoid races.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const KEYS: &[&str] = &[
        "LISTEN",
        "SIZE_LIMIT",
        "BUFFER_SIZE",
        "UPSTREAM_TIMEOUT",
        "SHUTDOWN_TIMEOUT",
    ];

    fn clear_all() {
        for k in KEYS {
            env::remove_var(k);
        }
    }

    #[test]
    fn defaults_when_env_unset() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_all();

        let cfg = Config::from_env();
        assert_eq!(cfg.listen, ":8080");
        assert_eq!(cfg.size_limit, 1_072_668_082_176);
        assert_eq!(cfg.buffer_size, 32 * 1024);
        assert_eq!(cfg.upstream_timeout, Duration::from_secs(30));
        assert_eq!(cfg.shutdown_timeout, Duration::from_secs(10));

        clear_all();
    }

    #[test]
    fn overrides_from_env() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_all();

        env::set_var("LISTEN", ":9000");
        env::set_var("SIZE_LIMIT", "1000");
        env::set_var("UPSTREAM_TIMEOUT", "5s");

        let cfg = Config::from_env();
        assert_eq!(cfg.listen, ":9000");
        assert_eq!(cfg.size_limit, 1000);
        assert_eq!(cfg.upstream_timeout, Duration::from_secs(5));
        // Unset values keep defaults.
        assert_eq!(cfg.buffer_size, 32 * 1024);
        assert_eq!(cfg.shutdown_timeout, Duration::from_secs(10));

        clear_all();
    }

    #[test]
    fn invalid_duration_falls_back() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_all();

        env::set_var("UPSTREAM_TIMEOUT", "garbage");
        let cfg = Config::from_env();
        assert_eq!(cfg.upstream_timeout, Duration::from_secs(30));

        clear_all();
    }

    #[test]
    fn invalid_u64_falls_back() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_all();

        env::set_var("SIZE_LIMIT", "-1");
        let cfg = Config::from_env();
        assert_eq!(cfg.size_limit, 1_072_668_082_176);

        clear_all();
    }

    #[test]
    fn empty_string_falls_back() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_all();

        env::set_var("LISTEN", "");
        let cfg = Config::from_env();
        assert_eq!(cfg.listen, ":8080");

        clear_all();
    }
}
