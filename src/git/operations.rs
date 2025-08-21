use super::{get_git_info, GitMessage, PoolGuard};
use crossbeam_channel::Sender;
use std::path::PathBuf;

fn create_git_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("git");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    cmd
}

pub fn switch_branch(
    repo_path: &PathBuf,
    branch_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let _repo = gix::open(repo_path)?;
    if branch_name.starts_with("remotes/") {
        let parts: Vec<&str> = branch_name.split('/').collect();
        if parts.len() >= 3 {
            let local_branch_name = parts[2..].join("/");

            let check_local = create_git_command()
                .args(&[
                    "show-ref",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{}", local_branch_name),
                ])
                .current_dir(repo_path)
                .output()?;

            if check_local.status.success() {
                let output = create_git_command()
                    .args(&["checkout", &local_branch_name])
                    .current_dir(repo_path)
                    .output()?;

                if !output.status.success() {
                    return Err(format!(
                        "Git checkout failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )
                    .into());
                }

                println!("Switched to existing local branch: {}", local_branch_name);
            } else {
                let output = create_git_command()
                    .args(&["checkout", "-b", &local_branch_name, branch_name])
                    .current_dir(repo_path)
                    .output()?;

                if !output.status.success() {
                    return Err(format!(
                        "Git checkout failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )
                    .into());
                }

                println!(
                    "Created and switched to new tracking branch: {}",
                    local_branch_name
                );
            }
        } else {
            return Err("Invalid remote branch name format".into());
        }
    } else {
        let output = create_git_command()
            .args(&["checkout", branch_name])
            .current_dir(repo_path)
            .output()?;

        if !output.status.success() {
            return Err(format!(
                "Git checkout failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        println!("Switched to branch: {}", branch_name);
    }

    Ok(())
}

pub fn git_fetch(repo_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let output = create_git_command()
        .args(&["fetch"])
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Git fetch failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    println!("Fetched for repo: {:?}", repo_path);
    Ok(())
}

pub fn git_pull(repo_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let output = create_git_command()
        .args(&["pull"])
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Git pull failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    println!("Pulled for repo: {:?}", repo_path);
    Ok(())
}

pub fn git_push(repo_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let output = create_git_command()
        .args(&["push"])
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Git push failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    println!("Pushed for repo: {:?}", repo_path);
    Ok(())
}

pub fn git_reset_hard(repo_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let output = create_git_command()
        .args(&["reset", "--hard"])
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Git reset failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    println!("Reset hard for repo: {:?}", repo_path);
    Ok(())
}

pub fn refresh_repo_status_async<T>(repo_path: PathBuf, tx: Sender<T>)
where
    T: From<GitMessage> + Send + 'static,
{
    std::thread::spawn(move || {
        let start_time = std::time::Instant::now();
        let repo_name = repo_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        match get_git_info(&repo_path) {
            Ok(git_info) => {
                let elapsed = start_time.elapsed();
                println!("Git info loaded for {} in {:?}", repo_name, elapsed);

                let msg = GitMessage::RepoStatusUpdated {
                    repo_path,
                    git_info,
                };
                if tx.send(T::from(msg)).is_err() {
                    eprintln!("Failed to send git info update");
                }
            }
            Err(e) => {
                let elapsed = start_time.elapsed();
                println!("Git info failed for {} in {:?}: {}", repo_name, elapsed, e);

                let msg = GitMessage::Error(format!("Git error for {:?}: {}", repo_path, e));
                if tx.send(T::from(msg)).is_err() {
                    eprintln!("Failed to send error message");
                }
            }
        }
    });
}

pub fn git_pull_fast_async<T>(repo_path: PathBuf, tx: Sender<T>)
where
    T: From<GitMessage> + Send + 'static,
{
    std::thread::spawn(move || {
        let _guard = PoolGuard::acquire();

        let result = git_pull(&repo_path);

        match result {
            Ok(_) => match get_git_info(&repo_path) {
                Ok(git_info) => {
                    let msg = GitMessage::RepoStatusUpdated {
                        repo_path,
                        git_info,
                    };
                    let _ = tx.send(T::from(msg));
                }
                Err(e) => {
                    let msg = GitMessage::Error(format!(
                        "Failed to get git info after pull for {:?}: {}",
                        repo_path, e
                    ));
                    let _ = tx.send(T::from(msg));
                }
            },
            Err(e) => {
                let msg = GitMessage::Error(format!("Pull failed for {:?}: {}", repo_path, e));
                let _ = tx.send(T::from(msg));
            }
        }
    });
}

pub fn git_push_fast_async<T>(repo_path: PathBuf, tx: Sender<T>)
where
    T: From<GitMessage> + Send + 'static,
{
    std::thread::spawn(move || {
        let _guard = PoolGuard::acquire();

        let result = git_push(&repo_path);

        match result {
            Ok(_) => match get_git_info(&repo_path) {
                Ok(git_info) => {
                    let msg = GitMessage::RepoStatusUpdated {
                        repo_path,
                        git_info,
                    };
                    let _ = tx.send(T::from(msg));
                }
                Err(e) => {
                    let msg = GitMessage::Error(format!(
                        "Failed to get git info after push for {:?}: {}",
                        repo_path, e
                    ));
                    let _ = tx.send(T::from(msg));
                }
            },
            Err(e) => {
                let msg = GitMessage::Error(format!("Push failed for {:?}: {}", repo_path, e));
                let _ = tx.send(T::from(msg));
            }
        }
    });
}

pub fn git_fetch_fast_async<T>(repo_path: PathBuf, tx: Sender<T>)
where
    T: From<GitMessage> + Send + 'static,
{
    std::thread::spawn(move || {
        let _guard = PoolGuard::acquire();

        let result = git_fetch(&repo_path);

        match result {
            Ok(_) => match get_git_info(&repo_path) {
                Ok(git_info) => {
                    let msg = GitMessage::RepoStatusUpdated {
                        repo_path,
                        git_info,
                    };
                    let _ = tx.send(T::from(msg));
                }
                Err(e) => {
                    let msg = GitMessage::Error(format!(
                        "Failed to get git info after fetch for {:?}: {}",
                        repo_path, e
                    ));
                    let _ = tx.send(T::from(msg));
                }
            },
            Err(e) => {
                let msg = GitMessage::Error(format!("Fetch failed for {:?}: {}", repo_path, e));
                let _ = tx.send(T::from(msg));
            }
        }
    });
}

pub fn git_fetch_fast_async_with_retry<T>(repo_path: PathBuf, tx: Sender<T>)
where
    T: From<GitMessage> + Send + 'static,
{
    std::thread::spawn(move || {
        let _guard = match PoolGuard::try_acquire_with_timeout(5000) {
            Some(guard) => guard,
            None => {
                let msg = GitMessage::Error(format!(
                    "Timeout waiting for available slot for {:?}",
                    repo_path
                ));
                let _ = tx.send(T::from(msg));
                return;
            }
        };

        let mut attempt = 0;
        let max_attempts = 3;
        let mut delay_ms = 1000;

        while attempt < max_attempts {
            attempt += 1;

            let result = git_fetch(&repo_path);

            match result {
                Ok(_) => {
                    match get_git_info(&repo_path) {
                        Ok(git_info) => {
                            let msg = GitMessage::RepoStatusUpdated {
                                repo_path,
                                git_info,
                            };
                            let _ = tx.send(T::from(msg));
                        }
                        Err(e) => {
                            let msg = GitMessage::Error(format!(
                                "Failed to get git info after fetch for {:?}: {}",
                                repo_path, e
                            ));
                            let _ = tx.send(T::from(msg));
                        }
                    }
                    return;
                }
                Err(e) => {
                    let error_str = e.to_string();
                    if error_str.contains("Connection closed")
                        || error_str.contains("Connection refused")
                        || error_str.contains("Could not read from remote repository")
                    {
                        if attempt < max_attempts {
                            let retry_msg = GitMessage::Error(format!(
                                "Fetch failed for {:?} (attempt {}/{}), retrying in {}ms: {}",
                                repo_path, attempt, max_attempts, delay_ms, error_str
                            ));
                            let _ = tx.send(T::from(retry_msg));

                            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                            delay_ms *= 2;
                        } else {
                            let msg = GitMessage::Error(format!(
                                "Fetch failed for {:?} after {} attempts: {}",
                                repo_path, max_attempts, error_str
                            ));
                            let _ = tx.send(T::from(msg));
                        }
                    } else {
                        let msg = GitMessage::Error(format!(
                            "Fetch failed for {:?}: {}",
                            repo_path, error_str
                        ));
                        let _ = tx.send(T::from(msg));
                        return;
                    }
                }
            }
        }
    });
}
