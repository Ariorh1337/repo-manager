#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod git_logic;
mod icons;
mod ui_components;

use git_logic::{GitInfo, UiMessage, get_git_info, refresh_repo_status_async, switch_branch, git_reset_hard, switch_branch_fast, git_reset_hard_fast};
use icons::{IconManager, IconType};
use ui_components::{IconButton, icon_button, text_button, icon_text_button, icon_image};
use std::path::PathBuf;
use std::collections::HashSet;
use crossbeam_channel::{Receiver, Sender};

#[derive(Debug, Clone)]
struct LogEntry {
    timestamp: std::time::SystemTime,
    level: LogLevel,
    message: String,
}

#[derive(Debug, Clone)]
enum LogLevel {
    Info,
    Warning,
    Error,
}

impl LogLevel {
    fn color(&self) -> egui::Color32 {
        match self {
            LogLevel::Info => egui::Color32::LIGHT_GRAY,
            LogLevel::Warning => egui::Color32::YELLOW,
            LogLevel::Error => egui::Color32::LIGHT_RED,
        }
    }
    
    fn icon(&self) -> &str {
        match self {
            LogLevel::Info => "[I]",
            LogLevel::Warning => "[!]", 
            LogLevel::Error => "[E]",
        }
    }
}

#[derive(Debug, Clone)]
struct TreeNode {
    name: String,
    path: PathBuf,
    children: Vec<TreeNode>,
    repositories: Vec<(usize, PathBuf)>,
    is_expanded: bool,
}

impl TreeNode {
    fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            children: Vec::new(),
            repositories: Vec::new(),
            is_expanded: true,
        }
    }
    
    fn find_child_mut(&mut self, name: &str) -> Option<&mut TreeNode> {
        self.children.iter_mut().find(|child| child.name == name)
    }
    
    fn get_or_create_child(&mut self, name: String, path: PathBuf) -> &mut TreeNode {
        let exists = self.children.iter().any(|child| child.name == name);
        if !exists {
            self.children.push(TreeNode::new(name.clone(), path));
        }
        self.children.iter_mut().find(|child| child.name == name).unwrap()
    }
}

fn get_config_file_path() -> std::path::PathBuf {
    let legacy_config = std::path::PathBuf::from("repo_manager_config.json");
    if legacy_config.exists() {
        println!("Using legacy config location: {:?}", legacy_config);
        return legacy_config;
    }
    
    #[cfg(target_os = "macos")]
    {
        if let Some(home_dir) = std::env::var_os("HOME") {
            let mut path = std::path::PathBuf::from(home_dir);
            path.push("Library");
            path.push("Application Support");
            path.push("RepoManager");
            
            if let Err(_) = std::fs::create_dir_all(&path) {
                path = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
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
            let mut path = std::path::PathBuf::from(appdata);
            path.push("RepoManager");
            let _ = std::fs::create_dir_all(&path);
            path.push("config.json");
            return path;
        }
    }
    
    legacy_config
}

#[derive(Debug)]
enum AppMessage {
    Git(UiMessage),
    ReposFound { repos: Vec<PathBuf> },
    SearchComplete { total_found: usize },
}

impl From<UiMessage> for AppMessage {
    fn from(msg: UiMessage) -> Self {
        AppMessage::Git(msg)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Default, Clone)]
struct Workspace {
    name: String,
    repositories: Vec<RepositoryState>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
struct RepositoryState {
    path: PathBuf,
    #[serde(skip)]
    name: String,
    #[serde(skip)]
    git_info: GitInfo,
}

impl Default for RepositoryState {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            name: String::new(),
            git_info: GitInfo::default(),
        }
    }
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            workspaces: Vec::new(),
            active_workspace_idx: 0,
            app_receiver: None,
            app_sender: None,
            editing_workspace: None,
            new_workspace_name: String::new(),
            search_status: None,
            search_status_timer: None,
            is_searching: false,
            is_loading_on_startup: false,
            startup_loaded_repos: 0,
            syncing_repos: HashSet::new(),
            window_width: None,
            window_height: None,
            sidebar_width: 250.0,
            search_query: String::new(),
            sort_by_name: false,
            collapsed_paths: HashSet::new(),
            logs: Vec::new(),
            show_logs: false,
            first_startup: true,
            pending_git_loads: 0,
            icon_manager: IconManager::new(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct MyApp {
    workspaces: Vec<Workspace>,
    #[serde(skip)]
    active_workspace_idx: usize,
    #[serde(skip)]
    app_receiver: Option<Receiver<AppMessage>>,
    #[serde(skip)]
    app_sender: Option<Sender<AppMessage>>,
    #[serde(skip)]
    editing_workspace: Option<usize>,
    #[serde(skip)]
    new_workspace_name: String,
    #[serde(skip)]
    search_status: Option<String>,
    #[serde(skip)]
    search_status_timer: Option<std::time::Instant>,
    #[serde(skip)]
    is_searching: bool,
    #[serde(skip)]
    is_loading_on_startup: bool,
    #[serde(skip)]
    startup_loaded_repos: usize,
    #[serde(skip)]
    syncing_repos: HashSet<PathBuf>,
    window_width: Option<f32>,
    window_height: Option<f32>,
    sidebar_width: f32,
    #[serde(skip)]
    search_query: String,
    sort_by_name: bool,
    #[serde(skip)]
    collapsed_paths: HashSet<String>,
    #[serde(skip)]
    logs: Vec<LogEntry>,
    #[serde(skip)]
    show_logs: bool,
    #[serde(skip)]
    first_startup: bool,
    #[serde(skip)]
    pending_git_loads: usize,
    #[serde(skip)]
    icon_manager: IconManager,
}

fn main() {
    let mut app = MyApp::load_or_default();
    app.setup_git_communication();
    
    app.refresh_all_loaded_repos();
    
    let mut native_options = eframe::NativeOptions::default();
    
    if let (Some(width), Some(height)) = (app.window_width, app.window_height) {
        native_options.viewport.inner_size = Some(egui::Vec2::new(width, height));
    } else {
        native_options.viewport.inner_size = Some(egui::Vec2::new(1200.0, 800.0));
    }
    
    eframe::run_native("Repo Manager", native_options, Box::new(|_cc| Box::new(app))).unwrap();
}

impl MyApp {
    fn load_or_default() -> Self {
        let config_path = get_config_file_path();
        println!("Looking for config at: {:?}", config_path);
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            println!("Config loaded successfully from: {:?}", config_path);
            if let Ok(mut app) = serde_json::from_str::<MyApp>(&content) {
                for workspace in &mut app.workspaces {
                    for repo in &mut workspace.repositories {
                        repo.name = repo.path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                    }
                }
                
                app.first_startup = true;
                return app;
            }
        } else {
            println!("Config file not found, using defaults");
        }
        MyApp::default()
    }

    fn save_config(&self) {
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let config_path = get_config_file_path();
            match std::fs::write(&config_path, content) {
                Ok(_) => {
                    println!("Config saved to: {:?}", config_path);
                }
                Err(e) => {
                    eprintln!("Failed to save config to {:?}: {}", config_path, e);
                }
            }
        }
    }

    fn setup_git_communication(&mut self) {
        let (tx, rx) = crossbeam_channel::unbounded::<AppMessage>();
        self.app_sender = Some(tx);
        self.app_receiver = Some(rx);
    }

    fn refresh_all_repos(&self) {
        if let Some(tx) = &self.app_sender {
            if let Some(workspace) = self.workspaces.get(self.active_workspace_idx) {
                for repo in &workspace.repositories {
                    refresh_repo_status_async::<AppMessage>(repo.path.clone(), tx.clone());
                }
            }
        }
    }
    
    fn refresh_all_loaded_repos(&mut self) {
        if let Some(tx) = &self.app_sender {
            let mut total_repos = 0;
            for workspace in &self.workspaces {
                for repo in &workspace.repositories {
                    refresh_repo_status_async::<AppMessage>(repo.path.clone(), tx.clone());
                    total_repos += 1;
                }
            }
            
            if total_repos > 0 {
                self.is_loading_on_startup = true;
                self.startup_loaded_repos = 0;
                self.search_status = Some(format!("Загрузка информации о {} репозиториях...", total_repos));
                self.search_status_timer = Some(std::time::Instant::now());
            }
        }
    }

    fn add_repository(&mut self, path: PathBuf) {
        self.log_info(format!("Searching for repositories in: {}", path.display()));
        self.search_status = Some(format!("Поиск репозиториев в {:?}...", path.file_name().unwrap_or_default()));
        self.search_status_timer = Some(std::time::Instant::now());
        self.is_searching = true;
        
        if let Some(tx) = &self.app_sender {
            let tx_clone = tx.clone();
            std::thread::spawn(move || {
                let repos = find_git_repositories_sync(&path);
                if let Err(_) = tx_clone.send(AppMessage::ReposFound { repos }) {}
            });
        }
    }
    
    fn find_git_repositories(&self, path: &PathBuf) -> Vec<PathBuf> {
        let mut repositories = Vec::new();

        if self.is_git_repository(path) {
            repositories.push(path.clone());
            return repositories;
        }

        self.scan_for_repositories(path, &mut repositories);
        
        repositories
    }
    
    fn is_git_repository(&self, path: &PathBuf) -> bool {
        path.join(".git").exists()
    }
    
    fn scan_for_repositories(&self, dir: &PathBuf, repositories: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                
                if path.is_dir() {
                    if self.is_git_repository(&path) {
                        repositories.push(path);
                    } else {
                        if let Some(name) = path.file_name() {
                            let name_str = name.to_string_lossy();
                            if !name_str.starts_with('.') && 
                               !name_str.eq_ignore_ascii_case("node_modules") &&
                               !name_str.eq_ignore_ascii_case("target") &&
                               !name_str.eq_ignore_ascii_case("build") {
                                self.scan_for_repositories(&path, repositories);
                            }
                        }
                    }
                }
            }
        }
    }

    fn get_repo_display_name(&self, repo_path: &PathBuf) -> String {
        repo_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }

    fn get_parent_group_name(&self, repo_path: &PathBuf) -> String {
        if let Some(parent) = repo_path.parent() {
            if let Some(parent_name) = parent.file_name() {
                return parent_name.to_string_lossy().to_string();
            }
        }
        "Прочее".to_string()
    }

    fn build_tree(&self, repositories: &[RepositoryState]) -> TreeNode {
        let mut root = TreeNode::new("Root".to_string(), PathBuf::new());
        
        for (idx, repo) in repositories.iter().enumerate() {
            let matches_search = if self.search_query.is_empty() {
                true
            } else {
                let query_lower = self.search_query.to_lowercase();
                repo.name.to_lowercase().contains(&query_lower) ||
                repo.path.to_string_lossy().to_lowercase().contains(&query_lower)
            };
            
            if !matches_search {
                continue;
            }

            let mut components: Vec<_> = repo.path.components()
                .filter_map(|comp| {
                    match comp {
                        std::path::Component::Normal(name) => Some(name.to_string_lossy().to_string()),
                        _ => None,
                    }
                })
                .collect();
            
            if components.is_empty() {
                continue;
            }

            let _repo_name = components.pop().unwrap();

            let mut current_node = &mut root;
            let mut current_path = PathBuf::new();
            
            for component in components {
                current_path.push(&component);
                current_node = current_node.get_or_create_child(component.clone(), current_path.clone());
            }

            current_node.repositories.push((idx, repo.path.clone()));
        }

        if self.sort_by_name {
            self.sort_tree_node(&mut root);
        }
        
        root
    }
    
    fn sort_tree_node(&self, node: &mut TreeNode) {
        node.children.sort_by(|a, b| a.name.cmp(&b.name));

        for child in &mut node.children {
            self.sort_tree_node(child);
        }

        node.repositories.sort_by(|a, b| {
            let repo_a = &self.workspaces.get(self.active_workspace_idx)
                .and_then(|ws| ws.repositories.get(a.0))
                .map(|r| &r.name);
            let repo_b = &self.workspaces.get(self.active_workspace_idx)
                .and_then(|ws| ws.repositories.get(b.0))
                .map(|r| &r.name);
            
            match (repo_a, repo_b) {
                (Some(a), Some(b)) => a.cmp(b),
                _ => std::cmp::Ordering::Equal,
            }
        });
    }
    
    fn render_tree_node(&mut self, ui: &mut egui::Ui, node: &TreeNode, workspace: &[RepositoryState], 
                       depth: usize, to_remove: &std::cell::RefCell<Option<usize>>) {
        if depth > 0 {
            let indent = (depth as f32) * 20.0;
            ui.horizontal(|ui| {
                ui.add_space(indent - 20.0);
                
                let has_children = !node.children.is_empty();
                let has_repos = !node.repositories.is_empty();
                
                if has_children || has_repos {
                    let node_path = node.path.to_string_lossy().to_string();
                    let is_collapsed = self.collapsed_paths.contains(&node_path);
                    let expand_symbol = if is_collapsed { "+" } else { "-" };
                    
                    if ui.button(format!("{} {}", expand_symbol, node.name)).clicked() {
                        if is_collapsed {
                            self.collapsed_paths.remove(&node_path);
                        } else {
                            self.collapsed_paths.insert(node_path.clone());
                        }
                    }

                    let total_items = node.children.len() + node.repositories.len();
                    if total_items > 0 {
                        ui.colored_label(egui::Color32::DARK_GRAY, format!("({} элементов)", total_items));
                    }
                } else {
                    ui.horizontal(|ui| {
                        icon_button(ui, &mut self.icon_manager, IconType::Folder);
                        ui.label(&node.name);
                    });
                }
            });

            let node_path = node.path.to_string_lossy().to_string();
            if self.collapsed_paths.contains(&node_path) && depth > 0 {
                return;
            }
        }

        for child in &node.children {
            self.render_tree_node(ui, child, workspace, depth + 1, to_remove);
        }

        for (original_idx, _) in &node.repositories {
            if let Some(repo) = workspace.get(*original_idx) {
                let indent = ((depth + 1) as f32) * 20.0;
                
                ui.horizontal(|ui| {
                    ui.add_space(indent);

                    let available_width = ui.available_width();
                    let menu_width = 30.0;
                    let status_width = 120.0;
                    let branch_width = f32::min(200.0, f32::max(120.0, available_width * 0.25));
                    let repo_width = available_width - branch_width - status_width - menu_width - 20.0;

                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(repo_width, 25.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_size(egui::Vec2::new(repo_width, 25.0));
                            if ui.button(&repo.name).clicked() {
                                opener::open(&repo.path).ok();
                            }
                        }
                    );

                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(branch_width, 25.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_size(egui::Vec2::new(branch_width, 25.0));
                            ui.set_max_size(egui::Vec2::new(branch_width, 25.0));

                            let current_branch = repo.git_info.current_branch.as_deref().unwrap_or("...");
                            let display_branch = if current_branch.len() > 15 {
                                format!("{}...", &current_branch[..12])
                            } else {
                                current_branch.to_string()
                            };
                            
                            egui::ComboBox::from_id_source(&repo.path)
                                .selected_text(display_branch)
                                .width(branch_width - 10.0)
                                .show_ui(ui, |ui| {
                                    for branch in &repo.git_info.branches {
                                        let label = ui.selectable_label(false, branch)
                                            .on_hover_text(branch);
                                        
                                        if label.clicked() {
                                            let switch_result = switch_branch_fast(&repo.path, branch)
                                                .or_else(|_| switch_branch(&repo.path, branch));
                                                
                                            if let Err(e) = switch_result {
                                                self.log_error(format!("Branch switch error for {}: {}", repo.name, e));
                                            } else {
                                                if let Some(tx) = &self.app_sender {
                                                    refresh_repo_status_async::<AppMessage>(repo.path.clone(), tx.clone());
                                                }
                                            }
                                        }
                                    }
                                });
                        }
                    );

                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(status_width, 25.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_size(egui::Vec2::new(status_width, 25.0));
                            
                            if self.syncing_repos.contains(&repo.path) {
                                ui.spinner(); 
                            }
                            
                            if repo.git_info.behind > 0 {
                                let pull_button = icon_text_button(ui, &mut self.icon_manager, IconType::Pull, format!("{}", repo.git_info.behind));
                                if pull_button.clicked() {
                                    self.log_info(format!("Starting pull for {}", repo.name));
                                    self.syncing_repos.insert(repo.path.clone());
                                    if let Some(tx) = &self.app_sender {
                                        git_logic::git_pull_fast_async::<AppMessage>(repo.path.clone(), tx.clone());
                                    }
                                }
                                pull_button.on_hover_text(format!("Pull: {} коммитов на сервере", repo.git_info.behind));
                            }
                            
                            if repo.git_info.ahead > 0 {
                                let push_button = icon_text_button(ui, &mut self.icon_manager, IconType::Push, format!("{}", repo.git_info.ahead));
                                if push_button.clicked() {
                                    self.log_info(format!("Starting push for {}", repo.name));
                                    self.syncing_repos.insert(repo.path.clone());
                                    if let Some(tx) = &self.app_sender {
                                        git_logic::git_push_fast_async::<AppMessage>(repo.path.clone(), tx.clone());
                                    }
                                }
                                push_button.on_hover_text(format!("Push: {} локальных коммитов", repo.git_info.ahead));
                            }

                            if repo.git_info.has_changes {
                                let changes_indicator = ui.colored_label(egui::Color32::YELLOW, "!");
                                changes_indicator.on_hover_text("Есть незакоммиченные изменения в рабочей директории");
                            }
                        }
                    );

                    ui.menu_button("»", |ui| {
                        if text_button(ui, &mut self.icon_manager, "Fetch").clicked() {
                            self.log_info(format!("Starting fetch for {}", repo.name));
                            self.syncing_repos.insert(repo.path.clone());
                            if let Some(tx) = &self.app_sender {
                                git_logic::git_fetch_fast_async::<AppMessage>(repo.path.clone(), tx.clone());
                            }
                            ui.close_menu();
                        }
                        if ui.button("Fetch & Rebase").clicked() {
                            println!("Fetch with rebase for {:?}", repo.path);
                            ui.close_menu();
                        }
                        if IconButton::icon(IconType::Refresh)
                            .show(ui, &mut self.icon_manager).clicked() {
                            if let Some(tx) = &self.app_sender {
                                refresh_repo_status_async::<AppMessage>(repo.path.clone(), tx.clone());
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Reset Changes").clicked() {
                            let reset_result = git_reset_hard_fast(&repo.path)
                                .or_else(|_| git_reset_hard(&repo.path));
                                
                            if let Err(e) = reset_result {
                                self.log_error(format!("Reset error for {}: {}", repo.name, e));
                            } else {
                                self.log_info(format!("Reset local changes for {}", repo.name));
                                if let Some(tx) = &self.app_sender {
                                    refresh_repo_status_async::<AppMessage>(repo.path.clone(), tx.clone());
                                }
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if icon_text_button(ui, &mut self.icon_manager, IconType::Trash, "Remove").clicked() {
                            *to_remove.borrow_mut() = Some(*original_idx);
                            ui.close_menu();
                        }
                    });
                });
                
                ui.add_space(1.0);
            }
        }
    }
    
    fn log_info(&mut self, message: String) {
        self.logs.push(LogEntry {
            timestamp: std::time::SystemTime::now(),
            level: LogLevel::Info,
            message,
        });

        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
    }
    
    fn log_warning(&mut self, message: String) {
        self.logs.push(LogEntry {
            timestamp: std::time::SystemTime::now(),
            level: LogLevel::Warning,
            message,
        });
        
        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
    }
    
    fn log_error(&mut self, message: String) {
        self.logs.push(LogEntry {
            timestamp: std::time::SystemTime::now(),
            level: LogLevel::Error,
            message,
        });
        
        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
    }
    
    fn clear_logs(&mut self) {
        self.logs.clear();
    }
}

fn find_git_repositories_sync(path: &PathBuf) -> Vec<PathBuf> {
    let mut repositories = Vec::new();

    if is_git_repository_sync(path) {
        repositories.push(path.clone());
        return repositories;
    }

    scan_for_repositories_sync(path, &mut repositories);
    
    repositories
}

fn is_git_repository_sync(path: &PathBuf) -> bool {
    path.join(".git").exists()
}

fn scan_for_repositories_sync(dir: &PathBuf, repositories: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            
            if path.is_dir() {
                if is_git_repository_sync(&path) {
                    repositories.push(path);
                } else {
                    if let Some(name) = path.file_name() {
                        let name_str = name.to_string_lossy();
                        if !name_str.starts_with('.') && 
                           !name_str.eq_ignore_ascii_case("node_modules") &&
                           !name_str.eq_ignore_ascii_case("target") &&
                           !name_str.eq_ignore_ascii_case("build") {
                            scan_for_repositories_sync(&path, repositories);
                        }
                    }
                }
            }
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.first_startup {
            self.first_startup = false;

            let total_repos: usize = self.workspaces.iter().map(|w| w.repositories.len()).sum();
            self.pending_git_loads = total_repos;
            
            self.log_info(format!("Starting async git info loading for {} repositories...", total_repos));

            if let Some(tx) = &self.app_sender {
                for workspace in &self.workspaces {
                    for repo in &workspace.repositories {
                        refresh_repo_status_async::<AppMessage>(repo.path.clone(), tx.clone());
                    }
                }
            }
        }

        let size = ctx.input(|i| i.screen_rect().size());
        if size.x > 0.0 && size.y > 0.0 {
            let current_width = self.window_width.unwrap_or(0.0);
            let current_height = self.window_height.unwrap_or(0.0);
            if (size.x - current_width).abs() > 1.0 || (size.y - current_height).abs() > 1.0 {
                self.window_width = Some(size.x);
                self.window_height = Some(size.y);

                if self.search_status_timer.is_none() || 
                   self.search_status_timer.unwrap().elapsed() > std::time::Duration::from_secs(1) {
                    self.save_config();
                }
            }
        }

        if let Some(timer) = self.search_status_timer {
            if timer.elapsed() > std::time::Duration::from_secs(3) {
                self.search_status = None;
                self.search_status_timer = None;
            }
        }

        let mut pending_logs = Vec::new();
        if let Some(rx) = &self.app_receiver {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    AppMessage::Git(UiMessage::RepoStatusUpdated { repo_path, git_info }) => {
                        self.syncing_repos.remove(&repo_path);

                        if self.pending_git_loads > 0 {
                            self.pending_git_loads -= 1;
                        }

                        if let Some(repo_name) = repo_path.file_name() {
                            if self.pending_git_loads == 0 {
                                pending_logs.push((LogLevel::Info, format!("All repositories loaded! Last: {}", repo_name.to_string_lossy())));
                            } else {
                                pending_logs.push((LogLevel::Info, format!("Loaded: {} ({} remaining)", repo_name.to_string_lossy(), self.pending_git_loads)));
                            }
                        }

                        for workspace in &mut self.workspaces {
                            if let Some(repo) = workspace.repositories.iter_mut().find(|r| r.path == repo_path) {
                                repo.git_info = git_info.clone();

                                if self.is_loading_on_startup {
                                    self.startup_loaded_repos += 1;
                                    let total_repos: usize = self.workspaces.iter()
                                        .map(|w| w.repositories.len())
                                        .sum();
                                    
                                    if self.startup_loaded_repos >= total_repos {
                                        self.is_loading_on_startup = false;
                                        self.search_status = Some("Все репозитории загружены".to_string());
                                        self.search_status_timer = Some(std::time::Instant::now());
                                    } else {
                                        self.search_status = Some(format!("Загружено {}/{} репозиториев", 
                                            self.startup_loaded_repos, total_repos));
                                    }
                                }
                                break;
                            }
                        }
                    }
                    AppMessage::Git(UiMessage::Error(err)) => {
                        pending_logs.push((LogLevel::Error, format!("Git error: {}", err)));

                        if let Some(start) = err.find('"') {
                            if let Some(end) = err[start+1..].find('"') {
                                let path_str = &err[start+1..start+1+end];
                                let path = PathBuf::from(path_str);
                                self.syncing_repos.remove(&path);
                            }
                        }

                        if self.is_loading_on_startup {
                            self.startup_loaded_repos += 1;
                            let total_repos: usize = self.workspaces.iter()
                                .map(|w| w.repositories.len())
                                .sum();
                            
                            if self.startup_loaded_repos >= total_repos {
                                self.is_loading_on_startup = false;
                                self.search_status = Some("Загрузка завершена (с ошибками)".to_string());
                                self.search_status_timer = Some(std::time::Instant::now());
                            } else {
                                self.search_status = Some(format!("Загружено {}/{} репозиториев", 
                                    self.startup_loaded_repos, total_repos));
                            }
                        }
                    }
                    AppMessage::ReposFound { repos } => {
                        self.is_searching = false;
                        if let Some(workspace) = self.workspaces.get_mut(self.active_workspace_idx) {
                            let mut added_count = 0;
                            for repo_path in repos {
                                if workspace.repositories.iter().any(|r| r.path == repo_path) {
                                    continue;
                                }

                                let name = repo_path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string();
                                let mut repo_state = RepositoryState {
                                    path: repo_path.clone(),
                                    name,
                                    git_info: GitInfo::default(),
                                };
                                if let Ok(git_info) = get_git_info(&repo_path) {
                                    repo_state.git_info = git_info;
                                }
                                workspace.repositories.push(repo_state);
                                added_count += 1;
                                                                 if let Some(tx) = &self.app_sender {
                                     refresh_repo_status_async::<AppMessage>(repo_path, tx.clone());
                                 }
                            }
                            if added_count > 0 {
                                self.save_config();
                                pending_logs.push((LogLevel::Info, format!("Added {} repositories", added_count)));
                                self.search_status = Some(format!("Добавлено {} репозиториев", added_count));
                            } else {
                                pending_logs.push((LogLevel::Warning, "No new repositories found".to_string()));
                                self.search_status = Some("Репозитории не найдены или уже добавлены".to_string());
                            }
                            self.search_status_timer = Some(std::time::Instant::now());
                        }
                    }
                    AppMessage::SearchComplete { total_found } => {
                        self.is_searching = false;
                        self.search_status = Some(format!("Найдено {} репозиториев", total_found));
                        self.search_status_timer = Some(std::time::Instant::now());
                    }
                }
            }
        }

        for (level, message) in pending_logs {
            match level {
                LogLevel::Info => self.log_info(message),
                LogLevel::Warning => self.log_warning(message),
                LogLevel::Error => self.log_error(message),
            }
        }

        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                for file in &i.raw.dropped_files {
                    if let Some(path) = &file.path {
                        if path.is_dir() {
                            if self.workspaces.is_empty() {
                                self.workspaces.push(Workspace {
                                    name: "Default Workspace".to_string(),
                                    repositories: vec![],
                                });
                                self.active_workspace_idx = 0;
                            }
                            self.add_repository(path.clone());
                        }
                    }
                }
            }
        });

        egui::SidePanel::left("workspaces_panel")
            .resizable(true)
            .default_width(self.sidebar_width)
            .width_range(200.0..=400.0)
            .show(ctx, |ui| {

            let new_width = ui.available_width();
            if (self.sidebar_width - new_width).abs() > 1.0 {
                self.sidebar_width = new_width;
            }
            ui.heading("Workspaces");
            
            let mut to_remove = None;
            let mut to_rename = None;
            let mut should_add_workspace = false;
            let mut should_refresh_all = false;
            
            for (idx, workspace) in self.workspaces.iter().enumerate() {
                ui.horizontal(|ui| {
                    if self.editing_workspace == Some(idx) {
                        ui.allocate_ui_with_layout(
                            egui::Vec2::new(ui.available_width() - 80.0, 20.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                let response = ui.text_edit_singleline(&mut self.new_workspace_name);
                                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    to_rename = Some((idx, self.new_workspace_name.clone()));
                                }
                            }
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_button(ui, &mut self.icon_manager, IconType::Cross).clicked() {
                                self.editing_workspace = None;
                            }
                            if icon_button(ui, &mut self.icon_manager, IconType::Check).clicked() {
                                to_rename = Some((idx, self.new_workspace_name.clone()));
                            }
                        });
                    } else {
                        if ui.selectable_value(&mut self.active_workspace_idx, idx, &workspace.name).clicked() {
                            self.active_workspace_idx = idx;
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_button(ui, &mut self.icon_manager, IconType::Trash).clicked() {
                                to_remove = Some(idx);
                            }
                            if icon_button(ui, &mut self.icon_manager, IconType::Edit).clicked() {
                                self.editing_workspace = Some(idx);
                                self.new_workspace_name = workspace.name.clone();
                            }
                        });
                    }
                });
            }
            
            if ui.button("+ New Workspace").clicked() {
                should_add_workspace = true;
            }

            ui.separator();
            
            if ui.button("Refresh All").clicked() {
                should_refresh_all = true;
            }

            if let Some((idx, new_name)) = to_rename {
                if let Some(ws) = self.workspaces.get_mut(idx) {
                    ws.name = new_name;
                    self.save_config();
                }
                self.editing_workspace = None;
            }
            
            if let Some(idx) = to_remove {
                self.workspaces.remove(idx);
                if self.active_workspace_idx >= self.workspaces.len() {
                    self.active_workspace_idx = self.workspaces.len().saturating_sub(1);
                }
                self.save_config();
            }
            
            if should_add_workspace {
                let new_workspace = Workspace {
                    name: format!("Workspace {}", self.workspaces.len() + 1),
                    repositories: vec![],
                };

                self.workspaces.push(new_workspace);
                self.save_config();
            }
            
            if should_refresh_all {
                self.refresh_all_repos();
            }

            if let Some(status) = &self.search_status {
                ui.separator();
                if self.is_searching || self.is_loading_on_startup {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        let color = if self.is_loading_on_startup {
                            egui::Color32::from_rgb(100, 150, 255)
                        } else {
                            egui::Color32::from_rgb(100, 150, 200)
                        };
                        ui.colored_label(color, status);
                    });
                } else {
                    ui.colored_label(egui::Color32::from_rgb(100, 150, 100), status);
                }
            }
        });

        if self.show_logs {
            egui::TopBottomPanel::bottom("logs_panel")
                .resizable(true)
                .default_height(200.0)
                .height_range(100.0..=400.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Logs");
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Clear").clicked() {
                                self.clear_logs();
                            }
                        });
                    });
                    
                    ui.separator();
                    
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, true])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for log_entry in &self.logs {
                                ui.horizontal(|ui| {
                                    ui.colored_label(log_entry.level.color(), log_entry.level.icon());

                                    if let Ok(duration) = log_entry.timestamp.elapsed() {
                                        let seconds = duration.as_secs();
                                        let time_text = if seconds < 60 {
                                            format!("{}s", seconds)
                                        } else if seconds < 3600 {
                                            format!("{}m", seconds / 60)
                                        } else {
                                            format!("{}h", seconds / 3600)
                                        };
                                        ui.colored_label(egui::Color32::DARK_GRAY, format!("[{}]", time_text));
                                    }

                                    ui.colored_label(log_entry.level.color(), &log_entry.message);
                                });
                            }
                        });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.workspaces.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label("Создайте workspace для начала работы");
                });
                return;
            }

            let mut should_fetch_all = false;

            if self.active_workspace_idx >= self.workspaces.len() {
                self.active_workspace_idx = self.workspaces.len().saturating_sub(1);
            }
            
            let workspace_name = self.workspaces.get(self.active_workspace_idx)
                .map(|w| w.name.clone())
                .unwrap_or_default();
            

            
            ui.horizontal(|ui| {
                ui.heading(&workspace_name);
                if ui.button("Fetch All").clicked() {
                    should_fetch_all = true;
                }
                
                ui.separator();

                let logs_button_text = if self.show_logs { "Hide Logs" } else { "Show Logs" };
                if ui.button(logs_button_text).clicked() {
                    self.show_logs = !self.show_logs;
                }

                if self.pending_git_loads > 0 {
                    ui.colored_label(egui::Color32::LIGHT_BLUE, format!("[LOADING] Git info... ({} left)", self.pending_git_loads));
                }

                if !self.logs.is_empty() {
                    let error_count = self.logs.iter().filter(|log| matches!(log.level, LogLevel::Error)).count();
                    let warning_count = self.logs.iter().filter(|log| matches!(log.level, LogLevel::Warning)).count();
                    
                    if error_count > 0 {
                        ui.colored_label(egui::Color32::LIGHT_RED, format!("[E] {}", error_count));
                    }
                    if warning_count > 0 {
                        ui.colored_label(egui::Color32::YELLOW, format!("[!] {}", warning_count));
                    }
                    ui.horizontal(|ui| {
                        icon_image(ui, &mut self.icon_manager, IconType::Info);
                        ui.colored_label(egui::Color32::LIGHT_GRAY, format!("{}", self.logs.len()));
                    });
                }
            });
            
            if should_fetch_all {
                if let Some(workspace) = self.workspaces.get(self.active_workspace_idx) {
                    let repo_count = workspace.repositories.len();
                    let repos: Vec<_> = workspace.repositories.iter().map(|r| r.path.clone()).collect();
                    
                    self.log_info(format!("Starting fetch for {} repositories", repo_count));
                    
                    for (index, repo_path) in repos.into_iter().enumerate() {
                        self.syncing_repos.insert(repo_path.clone());

                        let delay_ms = index as u64 * 200;
                        
                        if let Some(tx) = &self.app_sender {
                            let tx_clone = tx.clone();
                            std::thread::spawn(move || {
                                if delay_ms > 0 {
                                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                                }
                                git_logic::git_fetch_fast_async_with_retry::<AppMessage>(repo_path, tx_clone);
                            });
                        }
                    }
                }
            }

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Поиск:");
                ui.text_edit_singleline(&mut self.search_query);
                
                ui.separator();
                
                if ui.checkbox(&mut self.sort_by_name, "Сортировать по имени").changed() {
                    self.save_config();
                }
            });

            ui.separator();


            if self.workspaces.get(self.active_workspace_idx).map_or(true, |w| w.repositories.is_empty()) {
                ui.centered_and_justified(|ui| {
                    ui.label("Перетащите папки с репозиториями в это окно");
                });
                return;
            }

            let to_remove = std::cell::RefCell::new(None);
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if let Some(workspace) = self.workspaces.get(self.active_workspace_idx) {
                        let tree = self.build_tree(&workspace.repositories);
                        let repos = workspace.repositories.clone();

                        self.render_tree_node(ui, &tree, &repos, 0, &to_remove);
                    }
                });

            if let Some(idx) = to_remove.into_inner() {
                if let Some(workspace) = self.workspaces.get_mut(self.active_workspace_idx) {
                    workspace.repositories.remove(idx);
                    self.save_config();
                }
            }
        });
    }
}