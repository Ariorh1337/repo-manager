// git_logic.rs

use std::path::PathBuf;
use crossbeam_channel::Sender;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

// Пул для ограничения одновременных git операций
lazy_static::lazy_static! {
    static ref GIT_OPERATION_POOL: Arc<Mutex<VecDeque<()>>> = {
        let mut pool = VecDeque::new();
        // Ограничиваем до 8 одновременных операций
        for _ in 0..8 {
            pool.push_back(());
        }
        Arc::new(Mutex::new(pool))
    };
}

struct PoolGuard;
impl PoolGuard {
    fn acquire() -> Option<Self> {
        GIT_OPERATION_POOL.lock().ok()?.pop_front().map(|_| PoolGuard)
    }

	fn try_acquire_with_timeout(timeout_ms: u64) -> Option<Self> {
        let start = std::time::Instant::now();
        while start.elapsed().as_millis() < timeout_ms as u128 {
            if let Some(guard) = Self::acquire() {
                return Some(guard);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        None
    }
}
impl Drop for PoolGuard {
    fn drop(&mut self) {
        if let Ok(mut pool) = GIT_OPERATION_POOL.lock() {
            pool.push_back(());
        }
    }
}

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
pub enum UiMessage {
    RepoStatusUpdated { 
        repo_path: PathBuf, 
        git_info: GitInfo 
    },
    Error(String),
}

pub fn get_git_info(repo_path: &PathBuf) -> Result<GitInfo, Box<dyn std::error::Error>> {
    // Проверяем что это действительно git репозиторий
    if !repo_path.join(".git").exists() {
        return Err(format!("{:?} is not a git repository", repo_path).into());
    }
    
    let repo = gix::open(repo_path)?;
    
    // Получаем текущую ветку через git command line
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
    
    // Получаем все ветки (упрощенная версия)
    let mut branches = Vec::new();
    if let Ok(output) = create_git_command()
        .args(&["branch", "-a"])
        .current_dir(repo_path)
        .output() 
    {
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            let line = line.trim();
            if line.starts_with("* ") {
                branches.push(line[2..].to_string());
            } else if !line.starts_with("remotes/") && !line.is_empty() {
                branches.push(line.to_string());
            }
        }
    }
    
    // Упрощенная проверка изменений - проверяем git status
    let has_changes = if let Ok(output) = create_git_command()
        .args(&["status", "--porcelain"])
        .current_dir(repo_path)
        .output() 
    {
        !output.stdout.is_empty()
    } else {
        false
    };
    
    // Получаем ahead/behind (упрощенная версия)
    let (ahead, behind) = get_ahead_behind(&repo, &current_branch).unwrap_or((0, 0));
    
    Ok(GitInfo {
        current_branch,
        branches,
        ahead,
        behind,
        has_changes,
    })
}

fn get_ahead_behind(repo: &gix::Repository, current_branch: &Option<String>) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    if let Some(branch_name) = current_branch {
        // Используем git command line для получения ahead/behind
        if let Ok(output) = create_git_command()
            .args(&["rev-list", "--count", "--left-right", &format!("{}...origin/{}", branch_name, branch_name)])
            .current_dir(repo.git_dir().parent().unwrap_or(repo.git_dir()))
            .output() 
        {
            let output_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Some((ahead_str, behind_str)) = output_str.split_once('\t') {
                let ahead = ahead_str.parse::<usize>().unwrap_or(0);
                let behind = behind_str.parse::<usize>().unwrap_or(0);
                return Ok((ahead, behind));
            }
        }
    }
    
    Ok((0, 0))
}

// Обобщенная функция для отправки UiMessage
pub fn refresh_repo_status_async<T>(repo_path: PathBuf, tx: Sender<T>) 
where 
    T: From<UiMessage> + Send + 'static,
{
    std::thread::spawn(move || {
        match get_git_info(&repo_path) {
            Ok(git_info) => {
                let msg = UiMessage::RepoStatusUpdated { 
                    repo_path, 
                    git_info 
                };
                if let Err(_) = tx.send(T::from(msg)) {
                    eprintln!("Failed to send git info update");
                }
            },
            Err(e) => {
                let msg = UiMessage::Error(format!("Git error for {:?}: {}", repo_path, e));
                if let Err(_) = tx.send(T::from(msg)) {
                    eprintln!("Failed to send error message");
                }
            }
        }
    });
}

pub fn switch_branch(repo_path: &PathBuf, branch_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let _repo = gix::open(repo_path)?;
    
    // Упрощенная версия смены ветки - используем git command line
    let output = create_git_command()
        .args(&["checkout", branch_name])
        .current_dir(repo_path)
        .output()?;
    
    if !output.status.success() {
        return Err(format!("Git checkout failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    println!("Switched to branch: {}", branch_name);
    Ok(())
}

pub fn git_fetch(repo_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let output = create_git_command()
        .args(&["fetch"])
        .current_dir(repo_path)
        .output()?;
    
    if !output.status.success() {
        return Err(format!("Git fetch failed: {}", String::from_utf8_lossy(&output.stderr)).into());
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
        return Err(format!("Git pull failed: {}", String::from_utf8_lossy(&output.stderr)).into());
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
        return Err(format!("Git push failed: {}", String::from_utf8_lossy(&output.stderr)).into());
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
        return Err(format!("Git reset failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    println!("Reset hard for repo: {:?}", repo_path);
    Ok(())
}

// Асинхронные версии git операций
pub fn git_pull_async<T>(repo_path: PathBuf, tx: Sender<T>) 
where 
    T: From<UiMessage> + Send + 'static,
{
    std::thread::spawn(move || {
        let mut cmd = create_git_command();
        cmd.args(&["pull"])
           .current_dir(&repo_path);
        
        let result = cmd.output();
        
        match result {
            Ok(output) => {
                if output.status.success() {
                    println!("Pulled for repo: {:?}", repo_path);
                    // Обновляем git info после pull
                    match get_git_info(&repo_path) {
                        Ok(git_info) => {
                            let msg = UiMessage::RepoStatusUpdated { 
                                repo_path, 
                                git_info 
                            };
                            let _ = tx.send(T::from(msg));
                        },
                        Err(e) => {
                            let msg = UiMessage::Error(format!("Git info update failed after pull: {}", e));
                            let _ = tx.send(T::from(msg));
                        }
                    }
                } else {
                    let msg = UiMessage::Error(format!("Git pull failed for {:?}: {}", 
                        repo_path, String::from_utf8_lossy(&output.stderr)));
                    let _ = tx.send(T::from(msg));
                }
            },
            Err(e) => {
                let msg = UiMessage::Error(format!("Git pull command failed for {:?}: {}", repo_path, e));
                let _ = tx.send(T::from(msg));
            }
        }
    });
}

pub fn git_push_async<T>(repo_path: PathBuf, tx: Sender<T>) 
where 
    T: From<UiMessage> + Send + 'static,
{
    std::thread::spawn(move || {
        let mut cmd = create_git_command();
        cmd.args(&["push"])
           .current_dir(&repo_path);
        
        let result = cmd.output();
        
        match result {
            Ok(output) => {
                if output.status.success() {
                    println!("Pushed for repo: {:?}", repo_path);
                    // Обновляем git info после push
                    match get_git_info(&repo_path) {
                        Ok(git_info) => {
                            let msg = UiMessage::RepoStatusUpdated { 
                                repo_path, 
                                git_info 
                            };
                            let _ = tx.send(T::from(msg));
                        },
                        Err(e) => {
                            let msg = UiMessage::Error(format!("Git info update failed after push: {}", e));
                            let _ = tx.send(T::from(msg));
                        }
                    }
                } else {
                    let msg = UiMessage::Error(format!("Git push failed for {:?}: {}", 
                        repo_path, String::from_utf8_lossy(&output.stderr)));
                    let _ = tx.send(T::from(msg));
                }
            },
            Err(e) => {
                let msg = UiMessage::Error(format!("Git push command failed for {:?}: {}", repo_path, e));
                let _ = tx.send(T::from(msg));
            }
        }
    });
}

pub fn git_fetch_async<T>(repo_path: PathBuf, tx: Sender<T>) 
where 
    T: From<UiMessage> + Send + 'static,
{
    std::thread::spawn(move || {
        let mut cmd = create_git_command();
        cmd.args(&["fetch"])
           .current_dir(&repo_path);
        
        let result = cmd.output();
        
        match result {
            Ok(output) => {
                if output.status.success() {
                    println!("Fetched for repo: {:?}", repo_path);
                    // Обновляем git info после fetch
                    match get_git_info(&repo_path) {
                        Ok(git_info) => {
                            let msg = UiMessage::RepoStatusUpdated { 
                                repo_path, 
                                git_info 
                            };
                            let _ = tx.send(T::from(msg));
                        },
                        Err(e) => {
                            let msg = UiMessage::Error(format!("Git info update failed after fetch: {}", e));
                            let _ = tx.send(T::from(msg));
                        }
                    }
                } else {
                    let msg = UiMessage::Error(format!("Git fetch failed for {:?}: {}", 
                        repo_path, String::from_utf8_lossy(&output.stderr)));
                    let _ = tx.send(T::from(msg));
                }
            },
            Err(e) => {
                let msg = UiMessage::Error(format!("Git fetch command failed for {:?}: {}", repo_path, e));
                let _ = tx.send(T::from(msg));
            }
        }
    });
}

// Быстрые gix версии основных операций (пока fallback на git команды, но с пулом)
pub fn git_fetch_fast(repo_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Пока используем git команду, но в будущем можно заменить на pure gix
    let output = create_git_command()
        .args(&["fetch"])
        .current_dir(repo_path)
        .output()?;
        
    if !output.status.success() {
        return Err(format!("Git fetch failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    Ok(())
}

pub fn git_pull_fast(repo_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let output = create_git_command()
        .args(&["pull"])
        .current_dir(repo_path)
        .output()?;
        
    if !output.status.success() {
        return Err(format!("Git pull failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    Ok(())
}

pub fn git_push_fast(repo_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let output = create_git_command()
        .args(&["push"])
        .current_dir(repo_path)
        .output()?;
        
    if !output.status.success() {
        return Err(format!("Git push failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    Ok(())
}

pub fn switch_branch_fast(repo_path: &PathBuf, branch_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = create_git_command()
        .args(&["checkout", branch_name])
        .current_dir(repo_path)
        .output()?;
    
    if !output.status.success() {
        return Err(format!("Git checkout failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    Ok(())
}

pub fn git_reset_hard_fast(repo_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let output = create_git_command()
        .args(&["reset", "--hard"])
        .current_dir(repo_path)
        .output()?;
    
    if !output.status.success() {
        return Err(format!("Git reset failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    Ok(())
}

// Вспомогательная функция для создания git команд с правильными флагами на Windows
fn create_git_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    
    // На Windows в GUI режиме нужно скрыть окна процессов
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    
    cmd
}

// Быстрые асинхронные версии с пулом операций
pub fn git_pull_fast_async<T>(repo_path: PathBuf, tx: Sender<T>) 
where 
    T: From<UiMessage> + Send + 'static,
{
    std::thread::spawn(move || {
        let _guard = PoolGuard::acquire();
        
        // Используем быструю версию с пулом
        let result = git_pull_fast(&repo_path);
        
        match result {
            Ok(_) => {
                match get_git_info(&repo_path) {
                    Ok(git_info) => {
                        let msg = UiMessage::RepoStatusUpdated { 
                            repo_path, 
                            git_info 
                        };
                        let _ = tx.send(T::from(msg));
                    }
                    Err(e) => {
                        let msg = UiMessage::Error(format!("Failed to get git info after pull for {:?}: {}", repo_path, e));
                        let _ = tx.send(T::from(msg));
                    }
                }
            }
            Err(e) => {
                let msg = UiMessage::Error(format!("Pull failed for {:?}: {}", repo_path, e));
                let _ = tx.send(T::from(msg));
            }
        }
    });
}

pub fn git_push_fast_async<T>(repo_path: PathBuf, tx: Sender<T>) 
where 
    T: From<UiMessage> + Send + 'static,
{
    std::thread::spawn(move || {
        let _guard = PoolGuard::acquire();
        
        // Используем быструю версию с пулом
        let result = git_push_fast(&repo_path);
        
        match result {
            Ok(_) => {
                match get_git_info(&repo_path) {
                    Ok(git_info) => {
                        let msg = UiMessage::RepoStatusUpdated { 
                            repo_path, 
                            git_info 
                        };
                        let _ = tx.send(T::from(msg));
                    }
                    Err(e) => {
                        let msg = UiMessage::Error(format!("Failed to get git info after push for {:?}: {}", repo_path, e));
                        let _ = tx.send(T::from(msg));
                    }
                }
            }
            Err(e) => {
                let msg = UiMessage::Error(format!("Push failed for {:?}: {}", repo_path, e));
                let _ = tx.send(T::from(msg));
            }
        }
    });
}

pub fn git_fetch_fast_async<T>(repo_path: PathBuf, tx: Sender<T>) 
where 
    T: From<UiMessage> + Send + 'static,
{
    std::thread::spawn(move || {
        let _guard = PoolGuard::acquire();
        
        // Используем быструю версию с пулом
        let result = git_fetch_fast(&repo_path);
        
        match result {
            Ok(_) => {
                match get_git_info(&repo_path) {
                    Ok(git_info) => {
                        let msg = UiMessage::RepoStatusUpdated { 
                            repo_path, 
                            git_info 
                        };
                        let _ = tx.send(T::from(msg));
                    }
                    Err(e) => {
                        let msg = UiMessage::Error(format!("Failed to get git info after fetch for {:?}: {}", repo_path, e));
                        let _ = tx.send(T::from(msg));
                    }
                }
            }
            Err(e) => {
                let msg = UiMessage::Error(format!("Fetch failed for {:?}: {}", repo_path, e));
                let _ = tx.send(T::from(msg));
            }
        }
    });
}

pub fn git_fetch_fast_async_with_retry<T>(repo_path: PathBuf, tx: Sender<T>) 
where 
    T: From<UiMessage> + Send + 'static,
{
    std::thread::spawn(move || {
        // Ждем доступности слота в пуле с таймаутом
        let _guard = match PoolGuard::try_acquire_with_timeout(5000) {
            Some(guard) => guard,
            None => {
                let msg = UiMessage::Error(format!("Timeout waiting for available slot for {:?}", repo_path));
                let _ = tx.send(T::from(msg));
                return;
            }
        };
        
        // Retry логика с экспоненциальной задержкой
        let mut attempt = 0;
        let max_attempts = 3;
        let mut delay_ms = 1000; // Начинаем с 1 секунды
        
        while attempt < max_attempts {
            attempt += 1;
            
            let result = git_fetch_fast(&repo_path);
            
            match result {
                Ok(_) => {
                    match get_git_info(&repo_path) {
                        Ok(git_info) => {
                            let msg = UiMessage::RepoStatusUpdated { 
                                repo_path, 
                                git_info 
                            };
                            let _ = tx.send(T::from(msg));
                        }
                        Err(e) => {
                            let msg = UiMessage::Error(format!("Failed to get git info after fetch for {:?}: {}", repo_path, e));
                            let _ = tx.send(T::from(msg));
                        }
                    }
                    return; // Успех, выходим
                }
                Err(e) => {
                    let error_str = e.to_string();
                    // Проверяем на ошибки подключения
                    if error_str.contains("Connection closed") || 
                       error_str.contains("Connection refused") ||
                       error_str.contains("Could not read from remote repository") {
                        
                        if attempt < max_attempts {
                            // Логируем попытку повтора
                            let retry_msg = UiMessage::Error(format!("Fetch failed for {:?} (attempt {}/{}), retrying in {}ms: {}", 
                                repo_path, attempt, max_attempts, delay_ms, error_str));
                            let _ = tx.send(T::from(retry_msg));
                            
                            // Ждем перед повтором
                            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                            delay_ms *= 2; // Экспоненциальная задержка
                        } else {
                            // Финальная ошибка
                            let msg = UiMessage::Error(format!("Fetch failed for {:?} after {} attempts: {}", 
                                repo_path, max_attempts, error_str));
                            let _ = tx.send(T::from(msg));
                        }
                    } else {
                        // Не ошибка подключения, не повторяем
                        let msg = UiMessage::Error(format!("Fetch failed for {:?}: {}", repo_path, error_str));
                        let _ = tx.send(T::from(msg));
                        return;
                    }
                }
            }
        }
    });
}