#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod app;
mod config;
mod git;
mod localization;
mod logging;
mod ui;
mod workspace;

use app::{AppMessage, MyApp, RepositorySearcher, TreeBuilder};

use git::{
    git_fetch_fast_async, git_fetch_fast_async_with_retry, git_pull_fast_async,
    git_push_fast_async, git_reset_hard, refresh_repo_status_async, switch_branch, GitMessage,
};

use logging::LogLevel;
use ui::{Button, Icon, IconType};
use workspace::{RepositoryState, Workspace};

use std::path::PathBuf;

fn main() {
    let mut app = MyApp::load_or_default();
    app.setup_git_communication();

    let mut native_options = eframe::NativeOptions::default();

    if let (Some(width), Some(height)) = (app.config.window_width, app.config.window_height) {
        native_options.viewport.inner_size = Some(egui::Vec2::new(width, height));
    } else {
        native_options.viewport.inner_size = Some(egui::Vec2::new(1200.0, 800.0));
    }

    eframe::run_native(
        "Repo Manager",
        native_options,
        Box::new(|_cc| Box::new(app)),
    )
    .unwrap();
}

impl MyApp {
    fn add_repository(&mut self, path: PathBuf) {
        self.logger.info(
            self.localizer
                .tf("searching_in_path", &[&path.display().to_string()]),
        );
        self.search_status = Some(self.localizer.tf(
            "searching_repos",
            &[&format!("{:?}", path.file_name().unwrap_or_default())],
        ));
        self.search_status_timer = Some(std::time::Instant::now());
        self.is_searching = true;

        if let Some(tx) = &self.app_sender {
            let tx_clone = tx.clone();
            std::thread::spawn(move || {
                let repos = RepositorySearcher::find_git_repositories(&path);
                if tx_clone.send(AppMessage::ReposFound { repos }).is_err() {
                    eprintln!("Failed to send found repositories");
                }
            });
        }
    }

    fn render_tree_node(
        &mut self,
        ui: &mut egui::Ui,
        node: &app::TreeNode,
        workspace: &[RepositoryState],
        depth: usize,
        to_remove: &std::cell::RefCell<Option<usize>>,
    ) {
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

                    if ui
                        .button(format!("{} {}", expand_symbol, node.name))
                        .clicked()
                    {
                        if is_collapsed {
                            self.collapsed_paths.remove(&node_path);
                        } else {
                            self.collapsed_paths.insert(node_path.clone());
                        }
                    }

                    let total_items = node.children.len() + node.repositories.len();
                    if total_items > 0 {
                        ui.colored_label(
                            egui::Color32::DARK_GRAY,
                            self.localizer
                                .tf("elements_count", &[&total_items.to_string()]),
                        );
                    }
                } else {
                    ui.horizontal(|ui| {
                        Button::icon(IconType::Folder).show(ui, &mut self.icon_manager);
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

        let repos_count = node.repositories.len();
        for (repo_index, (original_idx, _)) in node.repositories.iter().enumerate() {
            if let Some(repo) = workspace.get(*original_idx) {
                let indent = ((depth + 1) as f32) * 20.0;

                ui.horizontal(|ui| {
                    ui.add_space(indent);

                    let available_width = ui.available_width();
                    let fetch_button_width = 30.0;
                    let menu_width = 35.0;
                    let status_width = 130.0;
                    let branch_width = f32::min(180.0, f32::max(100.0, available_width * 0.2));

                    let buttons_width = fetch_button_width + menu_width + 10.0;
                    let min_repo_width = 100.0;

                    let repo_width = f32::max(
                        min_repo_width,
                        available_width - branch_width - status_width - buttons_width,
                    );

                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(repo_width, 25.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_size(egui::Vec2::new(repo_width, 25.0));
                            if ui.button(&repo.name).clicked() {
                                opener::open(&repo.path).ok();
                            }
                        },
                    );

                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(branch_width, 25.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_size(egui::Vec2::new(branch_width, 25.0));
                            ui.set_max_size(egui::Vec2::new(branch_width, 25.0));

                            let current_branch =
                                repo.git_info.current_branch.as_deref().unwrap_or("...");
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
                                        let label = ui
                                            .selectable_label(false, branch)
                                            .on_hover_text(branch);

                                        if label.clicked() {
                                            if let Err(e) = switch_branch(&repo.path, branch) {
                                                self.logger.error(self.localizer.tf(
                                                    "branch_switch_error",
                                                    &[&repo.name, &e.to_string()],
                                                ));
                                            } else {
                                                if let Some(tx) = &self.app_sender {
                                                    refresh_repo_status_async::<AppMessage>(
                                                        repo.path.clone(),
                                                        tx.clone(),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                });
                        },
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
                                let pull_button = Button::icon_text(
                                    IconType::Pull,
                                    format!("{}", repo.git_info.behind),
                                )
                                .show(ui, &mut self.icon_manager);
                                if pull_button.clicked() {
                                    self.logger
                                        .info(self.localizer.tf("starting_pull", &[&repo.name]));
                                    self.syncing_repos.insert(repo.path.clone());
                                    if let Some(tx) = &self.app_sender {
                                        git_pull_fast_async::<AppMessage>(
                                            repo.path.clone(),
                                            tx.clone(),
                                        );
                                    }
                                }
                                pull_button.on_hover_text(
                                    self.localizer
                                        .tf("pull_commits", &[&repo.git_info.behind.to_string()]),
                                );
                            }

                            if repo.git_info.ahead > 0 {
                                let push_button = Button::icon_text(
                                    IconType::Push,
                                    format!("{}", repo.git_info.ahead),
                                )
                                .show(ui, &mut self.icon_manager);
                                if push_button.clicked() {
                                    self.logger
                                        .info(self.localizer.tf("starting_push", &[&repo.name]));
                                    self.syncing_repos.insert(repo.path.clone());
                                    if let Some(tx) = &self.app_sender {
                                        git_push_fast_async::<AppMessage>(
                                            repo.path.clone(),
                                            tx.clone(),
                                        );
                                    }
                                }
                                push_button.on_hover_text(
                                    self.localizer
                                        .tf("push_commits", &[&repo.git_info.ahead.to_string()]),
                                );
                            }

                            if self.error_repos.contains(&repo.path) {
                                let error_indicator = ui.colored_label(egui::Color32::RED, "!");
                                error_indicator.on_hover_text(&self.localizer.t("error_loading"));
                            }

                            if !self.error_repos.contains(&repo.path) && repo.git_info.has_changes {
                                let changes_indicator =
                                    ui.colored_label(egui::Color32::YELLOW, "!");
                                changes_indicator.on_hover_text(&self.localizer.t("has_changes"));
                            }
                        },
                    );

                    if Button::icon(IconType::Refresh)
                        .show(ui, &mut self.icon_manager)
                        .on_hover_text(&self.localizer.t("fetch"))
                        .clicked()
                    {
                        self.logger
                            .info(self.localizer.tf("starting_fetch", &[&repo.name]));
                        self.syncing_repos.insert(repo.path.clone());
                        if let Some(tx) = &self.app_sender {
                            git_fetch_fast_async::<AppMessage>(repo.path.clone(), tx.clone());
                        }
                    }

                    ui.menu_button("»", |ui| {
                        if Button::icon_text(IconType::Refresh, &self.localizer.t("fetch"))
                            .full_width()
                            .show(ui, &mut self.icon_manager)
                            .clicked()
                        {
                            self.logger
                                .info(self.localizer.tf("starting_fetch", &[&repo.name]));
                            self.syncing_repos.insert(repo.path.clone());
                            if let Some(tx) = &self.app_sender {
                                git_fetch_fast_async::<AppMessage>(repo.path.clone(), tx.clone());
                            }
                            ui.close_menu();
                        }
                        if Button::icon_text(IconType::Refresh, &self.localizer.t("fetch_rebase"))
                            .full_width()
                            .show(ui, &mut self.icon_manager)
                            .clicked()
                        {
                            println!("Fetch with rebase for {:?}", repo.path);
                            ui.close_menu();
                        }
                        if Button::icon_text(IconType::Refresh, &self.localizer.t("refresh"))
                            .full_width()
                            .show(ui, &mut self.icon_manager)
                            .clicked()
                        {
                            self.error_repos.remove(&repo.path);
                            if let Some(tx) = &self.app_sender {
                                refresh_repo_status_async::<AppMessage>(
                                    repo.path.clone(),
                                    tx.clone(),
                                );
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if Button::icon_text(IconType::Cross, &self.localizer.t("reset_changes"))
                            .full_width()
                            .show(ui, &mut self.icon_manager)
                            .clicked()
                        {
                            if let Err(e) = git_reset_hard(&repo.path) {
                                self.logger.error(
                                    self.localizer
                                        .tf("reset_error", &[&repo.name, &e.to_string()]),
                                );
                            } else {
                                self.logger
                                    .info(self.localizer.tf("reset_success", &[&repo.name]));
                                if let Some(tx) = &self.app_sender {
                                    refresh_repo_status_async::<AppMessage>(
                                        repo.path.clone(),
                                        tx.clone(),
                                    );
                                }
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if Button::icon_text(IconType::Trash, &self.localizer.t("remove_repo"))
                            .full_width()
                            .show(ui, &mut self.icon_manager)
                            .clicked()
                        {
                            *to_remove.borrow_mut() = Some(*original_idx);
                            ui.close_menu();
                        }
                    });
                });

                if repo_index < repos_count - 1 {
                    ui.add_space(0.0);
                    let y_pos = ui.cursor().min.y;
                    let start_x = ui.cursor().min.x + indent + 10.0;
                    let available_width = ui.available_width() - (indent + 30.0);
                    let end_x = start_x + available_width;

                    let stroke = egui::Stroke::new(
                        0.5,
                        egui::Color32::from_rgba_unmultiplied(120, 120, 120, 80),
                    );
                    let dash_length = 3.0;
                    let gap_length = 2.0;

                    let mut current_x = start_x;
                    while current_x < end_x {
                        let dash_end = f32::min(current_x + dash_length, end_x);
                        ui.painter().line_segment(
                            [
                                egui::Pos2::new(current_x, y_pos),
                                egui::Pos2::new(dash_end, y_pos),
                            ],
                            stroke,
                        );
                        current_x += dash_length + gap_length;
                    }
                    ui.add_space(2.0);
                }
            }
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.first_startup {
            self.first_startup = false;

            if !self.config.workspaces.is_empty() {
                self.load_workspace(self.active_workspace_idx);

                if let Some(workspace) = self.config.workspaces.get(self.active_workspace_idx) {
                    self.logger.info(self.localizer.tf(
                        "loading_workspace",
                        &[&workspace.name, &workspace.repositories.len().to_string()],
                    ));
                }
            }
        }

        let size = ctx.input(|i| i.screen_rect().size());
        if size.x > 0.0 && size.y > 0.0 {
            let current_width = self.config.window_width.unwrap_or(0.0);
            let current_height = self.config.window_height.unwrap_or(0.0);
            if (size.x - current_width).abs() > 1.0 || (size.y - current_height).abs() > 1.0 {
                self.config.window_width = Some(size.x);
                self.config.window_height = Some(size.y);

                if self.search_status_timer.is_none()
                    || self.search_status_timer.unwrap().elapsed()
                        > std::time::Duration::from_secs(1)
                {
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
        let mut messages = Vec::new();

        if let Some(rx) = &self.app_receiver {
            while let Ok(msg) = rx.try_recv() {
                messages.push(msg);
            }
        }

        for msg in messages {
            match msg {
                AppMessage::Git(GitMessage::RepoStatusUpdated {
                    repo_path,
                    git_info,
                }) => {
                    self.syncing_repos.remove(&repo_path);
                    self.error_repos.remove(&repo_path);

                    if self.pending_git_loads > 0 {
                        self.pending_git_loads -= 1;
                    }

                    if let Some(repo_name) = repo_path.file_name() {
                        if self.pending_git_loads == 0 {
                            pending_logs.push((
                                LogLevel::Info,
                                self.localizer
                                    .tf("repo_loaded_last", &[&repo_name.to_string_lossy()]),
                            ));
                        } else {
                            pending_logs.push((
                                LogLevel::Info,
                                self.localizer.tf(
                                    "repo_loaded_remaining",
                                    &[
                                        &repo_name.to_string_lossy(),
                                        &self.pending_git_loads.to_string(),
                                    ],
                                ),
                            ));
                        }
                    }

                    for workspace in &mut self.config.workspaces {
                        if let Some(repo) = workspace.find_repository_mut(&repo_path) {
                            repo.update_git_info(git_info.clone());

                            if self.is_loading_on_startup {
                                self.startup_loaded_repos += 1;
                                let total_repos: usize = self
                                    .config
                                    .workspaces
                                    .iter()
                                    .map(|w| w.repositories.len())
                                    .sum();

                                if self.startup_loaded_repos >= total_repos {
                                    self.is_loading_on_startup = false;
                                    self.search_status = Some(self.localizer.t("all_repos_loaded"));
                                    self.search_status_timer = Some(std::time::Instant::now());
                                } else {
                                    self.search_status = Some(self.localizer.tf(
                                        "loaded_count",
                                        &[
                                            &self.startup_loaded_repos.to_string(),
                                            &total_repos.to_string(),
                                        ],
                                    ));
                                }
                            }
                            break;
                        }
                    }
                }
                AppMessage::Git(GitMessage::Error(err)) => {
                    pending_logs.push((LogLevel::Error, format!("Git error: {}", err)));

                    if let Some(start) = err.find('"') {
                        if let Some(end) = err[start + 1..].find('"') {
                            let path_str = &err[start + 1..start + 1 + end];
                            let path = PathBuf::from(path_str);
                            self.syncing_repos.remove(&path);
                            self.error_repos.insert(path);
                        }
                    }

                    if self.is_loading_on_startup {
                        self.startup_loaded_repos += 1;
                        let total_repos: usize = self
                            .config
                            .workspaces
                            .iter()
                            .map(|w| w.repositories.len())
                            .sum();

                        if self.startup_loaded_repos >= total_repos {
                            self.is_loading_on_startup = false;
                            self.search_status = Some(self.localizer.t("loading_complete_errors"));
                            self.search_status_timer = Some(std::time::Instant::now());
                        } else {
                            self.search_status = Some(self.localizer.tf(
                                "loaded_count",
                                &[
                                    &self.startup_loaded_repos.to_string(),
                                    &total_repos.to_string(),
                                ],
                            ));
                        }
                    }
                }
                AppMessage::ReposFound { repos } => {
                    self.is_searching = false;

                    let mut added_count = 0;
                    let mut repos_to_refresh = Vec::new();

                    if let Some(workspace) = self.get_active_workspace_mut() {
                        for repo_path in repos {
                            if workspace.add_repository(repo_path.clone()) {
                                added_count += 1;
                                repos_to_refresh.push(repo_path);
                            }
                        }
                    }

                    if let Some(tx) = &self.app_sender {
                        for repo_path in repos_to_refresh {
                            refresh_repo_status_async::<AppMessage>(repo_path, tx.clone());
                        }
                    }

                    if added_count > 0 {
                        self.save_config();
                        pending_logs.push((
                            LogLevel::Info,
                            self.localizer
                                .tf("added_repos_log", &[&added_count.to_string()]),
                        ));
                        self.search_status = Some(
                            self.localizer
                                .tf("added_repos", &[&added_count.to_string()]),
                        );
                    } else {
                        pending_logs
                            .push((LogLevel::Warning, self.localizer.t("no_new_repos_log")));
                        self.search_status = Some(self.localizer.t("no_repos_found"));
                    }
                    self.search_status_timer = Some(std::time::Instant::now());
                }
                AppMessage::SearchComplete { total_found } => {
                    self.is_searching = false;
                    self.search_status = Some(
                        self.localizer
                            .tf("found_repos", &[&total_found.to_string()]),
                    );
                    self.search_status_timer = Some(std::time::Instant::now());
                }
            }
        }

        for (level, message) in pending_logs {
            match level {
                LogLevel::Info => self.logger.info(message),
                LogLevel::Warning => self.logger.warning(message),
                LogLevel::Error => self.logger.error(message),
            }
        }

        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                for file in &i.raw.dropped_files {
                    if let Some(path) = &file.path {
                        if path.is_dir() {
                            if self.config.workspaces.is_empty() {
                                self.config
                                    .workspaces
                                    .push(Workspace::new("Default Workspace"));
                                self.active_workspace_idx = 0;
                            }
                            self.add_repository(path.clone());
                        }
                    }
                }
            }
        });

        let is_editing = self.editing_workspace.is_some();
        let mut panel = egui::SidePanel::left("workspaces_panel")
            .resizable(!is_editing)
            .default_width(self.config.sidebar_width)
            .width_range(200.0..=400.0)
            .min_width(200.0)
            .max_width(400.0);

        if is_editing {
            panel = panel.exact_width(self.config.sidebar_width);
        }

        panel.show(ctx, |ui| {
            let new_width = ui.available_width();
            if !is_editing && (self.config.sidebar_width - new_width).abs() > 1.0 {
                self.config.sidebar_width = new_width;
            }

            ui.set_max_width(self.config.sidebar_width);

            ui.heading(&self.localizer.t("workspaces"));

            let mut to_remove = None;
            let mut to_rename = None;
            let mut should_add_workspace = false;
            let mut switch_to_workspace_idx: Option<usize> = None;

            for (idx, workspace) in self.config.workspaces.iter().enumerate() {
                ui.horizontal(|ui| {
                    if self.editing_workspace == Some(idx) {
                        let available_width = ui.available_width();
                        let button_width = 50.0;
                        let input_width = available_width - button_width - 15.0;

                        ui.scope(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.style_mut().spacing.indent = 0.0;

                            ui.set_max_width(input_width);
                            ui.set_min_width(input_width);

                            let response = ui.add_sized(
                                [input_width, 20.0],
                                egui::TextEdit::singleline(&mut self.new_workspace_name)
                                    .desired_width(input_width)
                                    .clip_text(true),
                            );

                            if response.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                to_rename = Some((idx, self.new_workspace_name.clone()));
                            }
                        });

                        if Button::icon(IconType::Check)
                            .show(ui, &mut self.icon_manager)
                            .clicked()
                        {
                            to_rename = Some((idx, self.new_workspace_name.clone()));
                        }
                        if Button::icon(IconType::Cross)
                            .show(ui, &mut self.icon_manager)
                            .clicked()
                        {
                            self.editing_workspace = None;
                        }
                    } else {
                        let available_width = ui.available_width();
                        let button_width = 50.0;
                        let name_width = available_width - button_width;

                        ui.allocate_ui_with_layout(
                            egui::Vec2::new(name_width, 25.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                let mut temp_active_idx = self.active_workspace_idx;
                                if ui
                                    .selectable_value(&mut temp_active_idx, idx, &workspace.name)
                                    .clicked()
                                {
                                    if temp_active_idx != self.active_workspace_idx {
                                        switch_to_workspace_idx = Some(temp_active_idx);
                                    }
                                }
                            },
                        );

                        if Button::icon(IconType::Edit)
                            .show(ui, &mut self.icon_manager)
                            .clicked()
                        {
                            self.editing_workspace = Some(idx);
                            self.new_workspace_name = workspace.name.clone();
                        }
                        if Button::icon(IconType::Trash)
                            .show(ui, &mut self.icon_manager)
                            .clicked()
                        {
                            to_remove = Some(idx);
                        }
                    }
                });
            }

            if ui.button(&self.localizer.t("new_workspace")).clicked() {
                should_add_workspace = true;
            }

            ui.separator();

            if let Some((idx, new_name)) = to_rename {
                if let Some(ws) = self.config.workspaces.get_mut(idx) {
                    ws.name = new_name;
                    self.save_config();
                }
                self.editing_workspace = None;
            }

            if let Some(idx) = to_remove {
                self.config.workspaces.remove(idx);
                if self.active_workspace_idx >= self.config.workspaces.len() {
                    self.active_workspace_idx = self.config.workspaces.len().saturating_sub(1);
                }
                self.save_config();
            }

            if should_add_workspace {
                let new_workspace =
                    Workspace::new(format!("Workspace {}", self.config.workspaces.len() + 1));
                self.config.workspaces.push(new_workspace);
                self.save_config();
            }

            if let Some(idx) = switch_to_workspace_idx {
                self.logger
                    .info(self.localizer.tf("switch_workspace", &[&idx.to_string()]));
                self.switch_to_workspace(idx);
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
                        ui.heading(&self.localizer.t("logs"));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(&self.localizer.t("clear")).clicked() {
                                self.logger.clear();
                            }
                        });
                    });

                    ui.separator();

                    egui::ScrollArea::vertical()
                        .auto_shrink([false, true])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for log_entry in self.logger.logs() {
                                ui.horizontal(|ui| {
                                    ui.colored_label(
                                        log_entry.level.color(),
                                        log_entry.level.icon(),
                                    );

                                    if let Ok(duration) = log_entry.timestamp.elapsed() {
                                        let seconds = duration.as_secs();
                                        let time_text = if seconds < 60 {
                                            format!("{}s", seconds)
                                        } else if seconds < 3600 {
                                            format!("{}m", seconds / 60)
                                        } else {
                                            format!("{}h", seconds / 3600)
                                        };
                                        ui.colored_label(
                                            egui::Color32::DARK_GRAY,
                                            format!("[{}]", time_text),
                                        );
                                    }

                                    ui.colored_label(log_entry.level.color(), &log_entry.message);
                                });
                            }
                        });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.config.workspaces.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(&self.localizer.t("create_workspace"));
                });
                return;
            }

            let mut should_fetch_all = false;

            if self.active_workspace_idx >= self.config.workspaces.len() {
                self.active_workspace_idx = self.config.workspaces.len().saturating_sub(1);
            }

            let workspace_name = self
                .get_active_workspace()
                .map(|w| w.name.clone())
                .unwrap_or_default();

            let mut should_refresh_all = false;

            ui.horizontal(|ui| {
                ui.heading(&workspace_name);
                if ui.button(&self.localizer.t("fetch_all")).clicked() {
                    should_fetch_all = true;
                }
                if ui.button(&self.localizer.t("refresh_all")).clicked() {
                    should_refresh_all = true;
                }

                ui.separator();

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let current_language = self.localizer.get_language().to_string();
                    let languages: Vec<(String, String)> = self
                        .localizer
                        .get_available_languages()
                        .into_iter()
                        .map(|(code, name)| (code.to_string(), name))
                        .collect();
                    let selected_text = languages
                        .iter()
                        .find(|(code, _)| *code == current_language)
                        .map(|(_, name)| name.as_str())
                        .unwrap_or("English");

                    egui::ComboBox::from_label(&self.localizer.t("language"))
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            for (lang_code, lang_name) in languages {
                                if ui
                                    .selectable_label(current_language == lang_code, &lang_name)
                                    .clicked()
                                {
                                    self.localizer.set_language(&lang_code);
                                    self.config.language = lang_code.to_string();
                                    self.save_config();
                                }
                            }
                        });

                    ui.separator();

                    let logs_button_text = if self.show_logs {
                        self.localizer.t("hide_logs")
                    } else {
                        self.localizer.t("show_logs")
                    };
                    if ui.button(&logs_button_text).clicked() {
                        self.show_logs = !self.show_logs;
                    }

                    if self.pending_git_loads > 0 {
                        ui.colored_label(
                            egui::Color32::LIGHT_BLUE,
                            self.localizer
                                .tf("loading_git_info", &[&self.pending_git_loads.to_string()]),
                        );
                    }

                    if !self.logger.logs().is_empty() {
                        let error_count = self.logger.error_count();
                        let warning_count = self.logger.warning_count();

                        ui.horizontal(|ui| {
                            Icon::show(ui, &mut self.icon_manager, IconType::Info, None);
                            ui.colored_label(
                                egui::Color32::LIGHT_GRAY,
                                format!("{}", self.logger.total_count()),
                            );
                        });

                        if warning_count > 0 {
                            ui.colored_label(
                                egui::Color32::YELLOW,
                                format!("[!] {}", warning_count),
                            );
                        }
                        if error_count > 0 {
                            ui.colored_label(
                                egui::Color32::LIGHT_RED,
                                format!("[E] {}", error_count),
                            );
                        }
                    }
                });
            });

            if should_fetch_all {
                if let Some(workspace) = self.get_active_workspace() {
                    let repo_count = workspace.repository_count();
                    let repos: Vec<_> = workspace
                        .repositories
                        .iter()
                        .map(|r| r.path.clone())
                        .collect();

                    self.logger.info(
                        self.localizer
                            .tf("starting_fetch_all", &[&repo_count.to_string()]),
                    );

                    for (index, repo_path) in repos.into_iter().enumerate() {
                        self.syncing_repos.insert(repo_path.clone());

                        let delay_ms = index as u64 * 200;

                        if let Some(tx) = &self.app_sender {
                            let tx_clone = tx.clone();
                            std::thread::spawn(move || {
                                if delay_ms > 0 {
                                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                                }
                                git_fetch_fast_async_with_retry::<AppMessage>(repo_path, tx_clone);
                            });
                        }
                    }
                }
            }

            if should_refresh_all {
                self.refresh_all_repos();
            }

            ui.separator();

            ui.horizontal(|ui| {
                ui.label(&self.localizer.t("search"));
                ui.text_edit_singleline(&mut self.search_query);

                ui.separator();

                if ui
                    .checkbox(
                        &mut self.config.sort_by_name,
                        &self.localizer.t("sort_by_name"),
                    )
                    .changed()
                {
                    self.save_config();
                }
            });

            ui.separator();

            if self.get_active_workspace().map_or(true, |w| w.is_empty()) {
                ui.centered_and_justified(|ui| {
                    ui.label(&self.localizer.t("drag_folders"));
                });
                return;
            }

            let to_remove = std::cell::RefCell::new(None);
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if let Some(workspace) = self.get_active_workspace() {
                        let tree = TreeBuilder::build_tree(
                            &workspace.repositories,
                            &self.search_query,
                            self.config.sort_by_name,
                        );
                        let repos = workspace.repositories.clone();

                        self.render_tree_node(ui, &tree, &repos, 0, &to_remove);
                    }
                });

            if let Some(idx) = to_remove.into_inner() {
                if let Some(workspace) = self.get_active_workspace_mut() {
                    workspace.remove_repository(idx);
                    self.save_config();
                }
            }
        });
    }
}
