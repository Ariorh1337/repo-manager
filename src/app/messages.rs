use crate::git::GitMessage;
use std::path::PathBuf;

#[derive(Debug)]
pub enum AppMessage {
    Git(GitMessage),
    ReposFound { repos: Vec<PathBuf> },
    SearchComplete { total_found: usize },
}

impl From<GitMessage> for AppMessage {
    fn from(msg: GitMessage) -> Self {
        AppMessage::Git(msg)
    }
}
