use crate::git::GitInfo;
use std::path::PathBuf;

#[derive(serde::Deserialize, serde::Serialize, Default, Clone)]
pub struct Workspace {
    pub name: String,
    pub repositories: Vec<RepositoryState>,
    #[serde(skip)] // Не сохраняем состояние загрузки в файл
    pub is_loaded: bool,
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct RepositoryState {
    pub path: PathBuf,
    #[serde(skip)]
    pub name: String,
    #[serde(skip)]
    pub git_info: GitInfo,
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

impl RepositoryState {
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        Self {
            path,
            name,
            git_info: GitInfo::default(),
        }
    }

    pub fn update_git_info(&mut self, git_info: GitInfo) {
        self.git_info = git_info;
    }
}

impl Workspace {
    pub fn new<T: Into<String>>(name: T) -> Self {
        Self {
            name: name.into(),
            repositories: Vec::new(),
            is_loaded: false,
        }
    }

    pub fn add_repository(&mut self, repo_path: PathBuf) -> bool {
        if self.repositories.iter().any(|r| r.path == repo_path) {
            return false;
        }

        let repo_state = RepositoryState::new(repo_path);
        self.repositories.push(repo_state);
        true
    }

    pub fn remove_repository(&mut self, index: usize) -> Option<RepositoryState> {
        if index < self.repositories.len() {
            Some(self.repositories.remove(index))
        } else {
            None
        }
    }

    pub fn find_repository_mut(&mut self, path: &PathBuf) -> Option<&mut RepositoryState> {
        self.repositories.iter_mut().find(|r| r.path == *path)
    }

    pub fn repository_count(&self) -> usize {
        self.repositories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.repositories.is_empty()
    }

    pub fn mark_as_loaded(&mut self) {
        self.is_loaded = true;
    }

    pub fn mark_as_unloaded(&mut self) {
        self.is_loaded = false;
    }
}
