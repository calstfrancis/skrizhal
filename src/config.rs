use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Data lives in `~/.local/share/`, config in `~/.config/` — the shared
/// convention across Cal's GTK4/libadwaita apps.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    #[serde(default = "default_data_path")]
    pub data_path: PathBuf,
    /// Shown once on first run; reachable again later via the header menu.
    #[serde(default)]
    pub has_seen_field_guide: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            data_path: default_data_path(),
            has_seen_field_guide: false,
        }
    }
}

fn default_data_path() -> PathBuf {
    PathBuf::from(shellexpand::tilde("~/.local/share/skrizhal/cv-elements.yaml").into_owned())
}

fn config_file() -> PathBuf {
    let base = shellexpand::tilde("~/.config/skrizhal").into_owned();
    PathBuf::from(base).join("config.toml")
}

impl Config {
    pub fn load() -> Self {
        std::fs::read_to_string(config_file())
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = config_file();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }
}
