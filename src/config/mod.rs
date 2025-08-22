use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub workspaces: Vec<crate::workspace::Workspace>,
    #[serde(default)]
    pub window_width: Option<f32>,
    #[serde(default)]
    pub window_height: Option<f32>,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: f32,
    #[serde(default)]
    pub sort_by_name: bool,
    #[serde(default)]
    pub last_active_workspace_index: Option<usize>,
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_sidebar_width() -> f32 {
    250.0
}

fn default_language() -> String {
    "en".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            workspaces: Vec::new(),
            window_width: None,
            window_height: None,
            sidebar_width: 250.0,
            sort_by_name: false,
            last_active_workspace_index: None,
            language: "en".to_string(),
        }
    }
}

pub struct ConfigManager;

impl ConfigManager {
    pub fn get_config_file_path() -> PathBuf {
        let legacy_config = PathBuf::from("repo_manager_config.json");
        if legacy_config.exists() {
            println!("Using legacy config location: {:?}", legacy_config);
            return legacy_config;
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(home_dir) = std::env::var_os("HOME") {
                let mut path = PathBuf::from(home_dir);
                path.push("Library");
                path.push("Application Support");
                path.push("RepoManager");

                if std::fs::create_dir_all(&path).is_err() {
                    path = PathBuf::from(std::env::var_os("HOME").unwrap());
                    path.push(".repo_manager");
                    let _ = std::fs::create_dir_all(&path);
                }

                path.push("config.json");
                return path;
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Some(appdata) = std::env::var_os("APPDATA") {
                let mut path = PathBuf::from(appdata);
                path.push("RepoManager");
                let _ = std::fs::create_dir_all(&path);
                path.push("config.json");
                return path;
            }
        }

        legacy_config
    }

    pub fn load() -> Config {
        let config_path = Self::get_config_file_path();
        println!("Looking for config at: {:?}", config_path);

        if let Ok(content) = std::fs::read_to_string(&config_path) {
            println!("Config loaded successfully from: {:?}", config_path);
            if let Ok(config) = serde_json::from_str::<Config>(&content) {
                return config;
            }
        } else {
            println!("Config file not found, using defaults");
        }

        Config::default()
    }

    pub fn save(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(config)?;
        let config_path = Self::get_config_file_path();

        std::fs::write(&config_path, content)?;
        println!("Config saved to: {:?}", config_path);

        Ok(())
    }
}
