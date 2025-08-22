pub mod messages;
pub mod search;
pub mod tree;

use crossbeam_channel::{Receiver, Sender};
use std::collections::HashSet;
use std::path::PathBuf;

use crate::config::{Config, ConfigManager};
use crate::git::refresh_repo_status_async;
use crate::localization::Localizer;
use crate::logging::Logger;
use crate::ui::IconManager;
use crate::workspace::Workspace;

pub use messages::*;
pub use search::*;
pub use tree::*;

pub struct MyApp {
    pub config: Config,
    pub logger: Logger,
    pub icon_manager: IconManager,
    pub localizer: Localizer,

    pub active_workspace_idx: usize,
    pub editing_workspace: Option<usize>,
    pub new_workspace_name: String,

    pub app_receiver: Option<Receiver<AppMessage>>,
    pub app_sender: Option<Sender<AppMessage>>,

    pub search_query: String,
    pub collapsed_paths: HashSet<String>,
    pub show_logs: bool,
    pub search_status: Option<String>,
    pub search_status_timer: Option<std::time::Instant>,

    pub is_searching: bool,
    pub is_loading_on_startup: bool,
    pub startup_loaded_repos: usize,
    pub syncing_repos: HashSet<PathBuf>,
    pub error_repos: HashSet<PathBuf>,
    pub pending_git_loads: usize,
    pub first_startup: bool,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            config: Config::default(),
            logger: Logger::default(),
            icon_manager: IconManager::new(),
            localizer: Localizer::new("en"),

            active_workspace_idx: 0,
            editing_workspace: None,
            new_workspace_name: String::new(),

            app_receiver: None,
            app_sender: None,

            search_query: String::new(),
            collapsed_paths: HashSet::new(),
            show_logs: false,
            search_status: None,
            search_status_timer: None,

            is_searching: false,
            is_loading_on_startup: false,
            startup_loaded_repos: 0,
            syncing_repos: HashSet::new(),
            error_repos: HashSet::new(),
            pending_git_loads: 0,
            first_startup: true,
        }
    }
}

impl MyApp {
    pub fn load_or_default() -> Self {
        let config = ConfigManager::load();
        let mut app = Self {
            localizer: Localizer::new(&config.language),
            config,
            ..Default::default()
        };

        if let Some(last_index) = app.config.last_active_workspace_index {
            if last_index < app.config.workspaces.len() {
                app.active_workspace_idx = last_index;
            }
        }

        for workspace in &mut app.config.workspaces {
            for repo in &mut workspace.repositories {
                repo.name = repo
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
            }
        }

        app.first_startup = true;
        app
    }

    pub fn save_config(&self) {
        if let Err(e) = ConfigManager::save(&self.config) {
            eprintln!("Failed to save config: {}", e);
        }
    }

    pub fn switch_to_workspace(&mut self, workspace_idx: usize) {
        if workspace_idx >= self.config.workspaces.len() {
            self.logger.info(format!(
                "Switch to workspace {} skipped: out of bounds",
                workspace_idx
            ));
            return;
        }

        let workspace_name = &self.config.workspaces[workspace_idx].name;
        self.logger.info(format!(
            "Switching to workspace: {} (index {})",
            workspace_name, workspace_idx
        ));

        self.active_workspace_idx = workspace_idx;
        self.config.last_active_workspace_index = Some(workspace_idx);

        self.load_workspace(workspace_idx);

        self.save_config();
    }

    pub fn setup_git_communication(&mut self) {
        let (tx, rx) = crossbeam_channel::unbounded::<AppMessage>();
        self.app_sender = Some(tx);
        self.app_receiver = Some(rx);
    }

    pub fn refresh_all_repos(&self) {
        if let Some(tx) = &self.app_sender {
            if let Some(workspace) = self.config.workspaces.get(self.active_workspace_idx) {
                for repo in &workspace.repositories {
                    refresh_repo_status_async::<AppMessage>(repo.path.clone(), tx.clone());
                }
            }
        }
    }

    pub fn load_workspace(&mut self, workspace_idx: usize) {
        if workspace_idx >= self.config.workspaces.len() {
            self.logger.info(format!(
                "Load workspace {} failed: index out of bounds",
                workspace_idx
            ));
            return;
        }

        let workspace = &mut self.config.workspaces[workspace_idx];
        if workspace.is_loaded {
            self.logger
                .info(format!("Workspace '{}' already loaded", workspace.name));
            return;
        }

        let repo_count = workspace.repositories.len();
        self.logger.info(format!(
            "Loading workspace '{}' with {} repositories",
            workspace.name, repo_count
        ));

        if let Some(tx) = &self.app_sender {
            self.pending_git_loads += repo_count;

            for repo in &workspace.repositories {
                self.logger.info(format!(
                    "Starting async load for repo: {}",
                    repo.path.display()
                ));
                refresh_repo_status_async::<AppMessage>(repo.path.clone(), tx.clone());
            }
        } else {
            self.logger
                .info("No app_sender available for loading repositories");
        }

        workspace.mark_as_loaded();
        self.logger
            .info(format!("Workspace '{}' marked as loaded", workspace.name));
    }

    pub fn refresh_all_loaded_repos(&mut self) {
        if let Some(tx) = &self.app_sender {
            let mut total_repos = 0;
            for workspace in &self.config.workspaces {
                for repo in &workspace.repositories {
                    refresh_repo_status_async::<AppMessage>(repo.path.clone(), tx.clone());
                    total_repos += 1;
                }
            }

            if total_repos > 0 {
                self.is_loading_on_startup = true;
                self.startup_loaded_repos = 0;
                self.search_status = Some(format!(
                    "Загрузка информации о {} репозиториях...",
                    total_repos
                ));
                self.search_status_timer = Some(std::time::Instant::now());
            }
        }
    }

    pub fn get_active_workspace(&self) -> Option<&Workspace> {
        self.config.workspaces.get(self.active_workspace_idx)
    }

    pub fn get_active_workspace_mut(&mut self) -> Option<&mut Workspace> {
        self.config.workspaces.get_mut(self.active_workspace_idx)
    }
}
