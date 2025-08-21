use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct GitInfo {
    pub current_branch: Option<String>,
    pub branches: Vec<String>,
    pub ahead: usize,
    pub behind: usize,
    pub has_changes: bool,
}

impl Default for GitInfo {
    fn default() -> Self {
        Self {
            current_branch: None,
            branches: vec![],
            ahead: 0,
            behind: 0,
            has_changes: false,
        }
    }
}

#[derive(Debug)]
pub enum GitMessage {
    RepoStatusUpdated {
        repo_path: PathBuf,
        git_info: GitInfo,
    },
    Error(String),
}

pub fn get_git_info(repo_path: &PathBuf) -> Result<GitInfo, Box<dyn std::error::Error>> {
    if !repo_path.join(".git").exists() {
        return Err(format!("{:?} is not a git repository", repo_path).into());
    }

    let repo = gix::open(repo_path)?;

    let current_branch = if let Ok(output) = create_git_command()
        .args(&["branch", "--show-current"])
        .current_dir(repo_path)
        .output()
    {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch.is_empty() {
            None
        } else {
            Some(branch)
        }
    } else {
        None
    };

    let mut branches = Vec::new();
    let mut local_branches = Vec::new();
    let mut remote_branches = Vec::new();

    let remotes = get_remotes(repo_path);

    if let Ok(output) = create_git_command()
        .args(&["branch", "-a", "--sort=-committerdate"])
        .current_dir(repo_path)
        .output()
    {
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if line.starts_with("* ") {
                let branch_name = line[2..].to_string();
                if !local_branches.contains(&branch_name) {
                    local_branches.push(branch_name);
                }
            } else if line.starts_with("remotes/") {
                let remote_branch = line.to_string();
                if !remote_branch.contains("HEAD") {
                    remote_branches.push(remote_branch);
                }
            } else if !line.is_empty() {
                let local_branch = line.to_string();
                if !local_branches.contains(&local_branch) {
                    local_branches.push(local_branch);
                }
            }
        }
    }

    branches.extend(local_branches.clone());

    for remote_branch in remote_branches {
        let mut found_local = false;
        for remote_name in &remotes {
            if let Some(branch_name) =
                remote_branch.strip_prefix(&format!("remotes/{}/", remote_name))
            {
                if local_branches.contains(&branch_name.to_string()) {
                    found_local = true;
                    break;
                }
            }
        }

        if !found_local {
            branches.push(remote_branch);
        }
    }

    let has_changes = if let Ok(output) = create_git_command()
        .args(&["status", "--porcelain"])
        .current_dir(repo_path)
        .output()
    {
        !output.stdout.is_empty()
    } else {
        false
    };

    let (ahead, behind) = get_ahead_behind(&repo, &current_branch).unwrap_or((0, 0));

    Ok(GitInfo {
        current_branch,
        branches,
        ahead,
        behind,
        has_changes,
    })
}

fn get_ahead_behind(
    repo: &gix::Repository,
    current_branch: &Option<String>,
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    if let Some(branch_name) = current_branch {
        let repo_path = repo.git_dir().parent().unwrap_or(repo.git_dir());
        let remotes = get_remotes(&repo_path.to_path_buf());

        for remote_name in &remotes {
            let remote_branch = format!("{}/{}", remote_name, branch_name);

            let check_local_remote = create_git_command()
                .args(&["show-branch", &remote_branch])
                .current_dir(repo_path)
                .output();

            if let Ok(output) = check_local_remote {
                if output.status.success() {
                    let rev_list_result = create_git_command()
                        .args(&[
                            "rev-list",
                            "--count",
                            "--left-right",
                            &format!("{}...{}", branch_name, remote_branch),
                        ])
                        .current_dir(repo_path)
                        .output();

                    if let Ok(output) = rev_list_result {
                        if output.status.success() {
                            let output_str =
                                String::from_utf8_lossy(&output.stdout).trim().to_string();
                            if let Some((ahead_str, behind_str)) = output_str.split_once('\t') {
                                let ahead = ahead_str.parse::<usize>().unwrap_or(0);
                                let behind = behind_str.parse::<usize>().unwrap_or(0);
                                return Ok((ahead, behind));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok((0, 0))
}

fn create_git_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("git");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    cmd
}

fn get_remotes(repo_path: &PathBuf) -> Vec<String> {
    if let Ok(output) = create_git_command()
        .args(&["remote"])
        .current_dir(repo_path)
        .output()
    {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            return output_str
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|remote| !remote.is_empty())
                .collect();
        }
    }

    vec!["origin".to_string()]
}
