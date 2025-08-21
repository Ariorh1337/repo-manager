use std::path::PathBuf;

pub struct RepositorySearcher;

impl RepositorySearcher {
    pub fn find_git_repositories(path: &PathBuf) -> Vec<PathBuf> {
        let mut repositories = Vec::new();

        if Self::is_git_repository(path) {
            repositories.push(path.clone());
            return repositories;
        }

        Self::scan_for_repositories(path, &mut repositories);

        repositories
    }

    fn is_git_repository(path: &PathBuf) -> bool {
        path.join(".git").exists()
    }

    fn scan_for_repositories(dir: &PathBuf, repositories: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_dir() {
                    if Self::is_git_repository(&path) {
                        repositories.push(path);
                    } else {
                        if let Some(name) = path.file_name() {
                            let name_str = name.to_string_lossy();
                            if !name_str.starts_with('.')
                                && !name_str.eq_ignore_ascii_case("node_modules")
                                && !name_str.eq_ignore_ascii_case("target")
                                && !name_str.eq_ignore_ascii_case("build")
                            {
                                Self::scan_for_repositories(&path, repositories);
                            }
                        }
                    }
                }
            }
        }
    }
}
