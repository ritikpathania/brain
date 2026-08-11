//! Trait-based system appearance detection providers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Observed system appearance mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Appearance {
    /// Dark system appearance.
    #[default]
    Dark,
    /// Light system appearance.
    Light,
}

impl Appearance {
    /// Detects current system appearance on macOS or via environment overrides.
    pub fn detect_system() -> Self {
        if let Ok(val) = std::env::var("BRAIN_THEME").or_else(|_| std::env::var("THEME")) {
            match val.to_lowercase().as_str() {
                "light" => return Appearance::Light,
                "dark" => return Appearance::Dark,
                _ => {}
            }
        }

        if let Ok(cfg) = std::env::var("COLORFGBG") {
            let parts: Vec<&str> = cfg.split(';').collect();
            if parts.len() >= 2 {
                if let Ok(bg) = parts[parts.len() - 1].trim().parse::<u8>() {
                    if bg == 7 || bg == 15 || bg >= 231 {
                        return Appearance::Light;
                    } else if bg == 0 || bg == 8 || bg == 16 {
                        return Appearance::Dark;
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("defaults")
                .args(["read", "-g", "AppleInterfaceStyle"])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .to_lowercase();
                if stdout.contains("dark") {
                    return Appearance::Dark;
                }
            }
            Appearance::Light
        }

        #[cfg(not(target_os = "macos"))]
        {
            Appearance::Dark
        }
    }
}

/// Trait abstracting system appearance detection without renderer coupling.
pub trait AppearanceProvider: Send + Sync {
    /// Returns the active observed system appearance mode.
    fn appearance(&self) -> Appearance;
}

/// Static appearance provider returning a fixed appearance (useful for testing and overrides).
#[derive(Debug, Clone, Copy)]
pub struct StaticProvider {
    appearance: Appearance,
}

impl StaticProvider {
    /// Creates a new StaticProvider with the given fixed appearance.
    pub fn new(appearance: Appearance) -> Self {
        Self { appearance }
    }
}

impl AppearanceProvider for StaticProvider {
    fn appearance(&self) -> Appearance {
        self.appearance
    }
}

/// Non-blocking macOS appearance provider using asynchronous background polling.
pub struct MacOSPollingProvider {
    cached: Arc<std::sync::Mutex<(Appearance, Instant)>>,
    running: Arc<AtomicBool>,
}

impl Default for MacOSPollingProvider {
    fn default() -> Self {
        Self::new(Duration::from_secs(2))
    }
}

impl MacOSPollingProvider {
    /// Instantiates a new MacOSPollingProvider with specified polling interval.
    pub fn new(interval: Duration) -> Self {
        let cached = Arc::new(std::sync::Mutex::new((
            Self::query_system_appearance(),
            Instant::now(),
        )));
        let running = Arc::new(AtomicBool::new(true));

        let cached_clone = Arc::clone(&cached);
        let running_clone = Arc::clone(&running);

        std::thread::spawn(move || {
            while running_clone.load(Ordering::Relaxed) {
                std::thread::sleep(interval);
                let current = Self::query_system_appearance();
                if let Ok(mut lock) = cached_clone.lock() {
                    *lock = (current, Instant::now());
                }
            }
        });

        Self { cached, running }
    }

    /// Queries system appearance without blocking the main loop.
    fn query_system_appearance() -> Appearance {
        Appearance::detect_system()
    }
}

impl Drop for MacOSPollingProvider {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl AppearanceProvider for MacOSPollingProvider {
    fn appearance(&self) -> Appearance {
        if let Ok(lock) = self.cached.lock() {
            lock.0
        } else {
            Appearance::Dark
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_appearance_provider() {
        let dark_provider = StaticProvider::new(Appearance::Dark);
        assert_eq!(dark_provider.appearance(), Appearance::Dark);

        let light_provider = StaticProvider::new(Appearance::Light);
        assert_eq!(light_provider.appearance(), Appearance::Light);
    }
}
