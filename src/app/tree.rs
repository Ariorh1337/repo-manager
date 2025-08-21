use crate::workspace::RepositoryState;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub children: Vec<TreeNode>,
    pub repositories: Vec<(usize, PathBuf)>,
    pub is_expanded: bool,
}

impl TreeNode {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            children: Vec::new(),
            repositories: Vec::new(),
            is_expanded: true,
        }
    }

    pub fn find_child_mut(&mut self, name: &str) -> Option<&mut TreeNode> {
        self.children.iter_mut().find(|child| child.name == name)
    }

    pub fn get_or_create_child(&mut self, name: String, path: PathBuf) -> &mut TreeNode {
        let exists = self.children.iter().any(|child| child.name == name);
        if !exists {
            self.children.push(TreeNode::new(name.clone(), path));
        }
        self.children
            .iter_mut()
            .find(|child| child.name == name)
            .unwrap()
    }
}

pub struct TreeBuilder;

impl TreeBuilder {
    pub fn build_tree(
        repositories: &[RepositoryState],
        search_query: &str,
        sort_by_name: bool,
    ) -> TreeNode {
        let mut root = TreeNode::new("Root".to_string(), PathBuf::new());

        for (idx, repo) in repositories.iter().enumerate() {
            let matches_search = if search_query.is_empty() {
                true
            } else {
                let query_lower = search_query.to_lowercase();
                repo.name.to_lowercase().contains(&query_lower)
                    || repo
                        .path
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&query_lower)
            };

            if !matches_search {
                continue;
            }

            let mut components: Vec<_> = repo
                .path
                .components()
                .filter_map(|comp| match comp {
                    std::path::Component::Normal(name) => Some(name.to_string_lossy().to_string()),
                    _ => None,
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
                current_node =
                    current_node.get_or_create_child(component.clone(), current_path.clone());
            }

            current_node.repositories.push((idx, repo.path.clone()));
        }

        if sort_by_name {
            Self::sort_tree_node(&mut root, repositories);
        }

        root
    }

    fn sort_tree_node(node: &mut TreeNode, repositories: &[RepositoryState]) {
        node.children.sort_by(|a, b| a.name.cmp(&b.name));

        for child in &mut node.children {
            Self::sort_tree_node(child, repositories);
        }

        node.repositories.sort_by(|a, b| {
            let repo_a = repositories.get(a.0).map(|r| &r.name);
            let repo_b = repositories.get(b.0).map(|r| &r.name);

            match (repo_a, repo_b) {
                (Some(a), Some(b)) => a.cmp(b),
                _ => std::cmp::Ordering::Equal,
            }
        });
    }
}
