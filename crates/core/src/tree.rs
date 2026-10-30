use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Node {
    Group {
        name: String,
        children: Vec<Node>,
        #[serde(default)]
        open: bool,
    },
    Project {
        path: PathBuf,
        #[serde(default)]
        sandbox_sessions: bool,
    },
}

impl Node {
    pub fn project(path: impl Into<PathBuf>) -> Self {
        Node::Project { path: path.into(), sandbox_sessions: false }
    }

    pub fn group(name: &str) -> Self {
        Node::Group { name: name.to_string(), children: Vec::new(), open: true }
    }

    pub fn label(&self) -> String {
        match self {
            Node::Group { name, .. } => name.clone(),
            Node::Project { path, .. } => {
                path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
            }
        }
    }
}

/// Where a node sits: the indices from the top level down.
pub type At = Vec<usize>;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tree {
    pub nodes: Vec<Node>,
}

impl Tree {
    pub fn load(file: &Path) -> Tree {
        std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = file.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap_or_default())?;
        std::fs::rename(tmp, file)
    }

    pub fn get(&self, at: &[usize]) -> Option<&Node> {
        let (first, rest) = at.split_first()?;
        let mut node = self.nodes.get(*first)?;
        for i in rest {
            match node {
                Node::Group { children, .. } => node = children.get(*i)?,
                Node::Project { .. } => return None,
            }
        }
        Some(node)
    }

    pub fn get_mut(&mut self, at: &[usize]) -> Option<&mut Node> {
        let (first, rest) = at.split_first()?;
        let mut node = self.nodes.get_mut(*first)?;
        for i in rest {
            match node {
                Node::Group { children, .. } => node = children.get_mut(*i)?,
                Node::Project { .. } => return None,
            }
        }
        Some(node)
    }

    fn list_mut(&mut self, group: &[usize]) -> Option<&mut Vec<Node>> {
        if group.is_empty() {
            return Some(&mut self.nodes);
        }
        match self.get_mut(group)? {
            Node::Group { children, .. } => Some(children),
            Node::Project { .. } => None,
        }
    }

    /// Every project, with where it sits.
    pub fn projects(&self) -> Vec<(At, &Path)> {
        fn walk<'a>(nodes: &'a [Node], at: &mut At, out: &mut Vec<(At, &'a Path)>) {
            for (i, n) in nodes.iter().enumerate() {
                at.push(i);
                match n {
                    Node::Project { path, .. } => out.push((at.clone(), path)),
                    Node::Group { children, .. } => walk(children, at, out),
                }
                at.pop();
            }
        }
        let mut out = Vec::new();
        walk(&self.nodes, &mut Vec::new(), &mut out);
        out
    }

    /// Adds a folder as a project at the top level; a folder in the tree already isn't added again. Where it is.
    pub fn add_project(&mut self, path: &Path) -> At {
        if let Some((at, _)) = self.projects().into_iter().find(|(_, p)| *p == path) {
            return at;
        }
        self.nodes.push(Node::project(path));
        vec![self.nodes.len() - 1]
    }

    pub fn add_group(&mut self, name: &str) -> At {
        self.nodes.push(Node::group(name));
        vec![self.nodes.len() - 1]
    }

    pub fn rename(&mut self, at: &[usize], new: &str) {
        if let Some(Node::Group { name, .. }) = self.get_mut(at) {
            *name = new.to_string();
        }
    }

    /// Takes a node out; a group's contents move up to where it was.
    pub fn remove(&mut self, at: &[usize]) {
        let Some((last, parent)) = at.split_last() else { return };
        let Some(list) = self.list_mut(parent) else { return };
        if *last >= list.len() {
            return;
        }
        match list.remove(*last) {
            Node::Group { children, .. } => {
                for (k, c) in children.into_iter().enumerate() {
                    list.insert(*last + k, c);
                }
            }
            Node::Project { .. } => {}
        }
    }

    /// Moves a node into the group `into` (the top level when empty) at `index`; never into itself.
    pub fn move_to(&mut self, from: &[usize], into: &[usize], index: usize) {
        if into.starts_with(from) || from.is_empty() {
            return;
        }
        let Some((last, parent)) = from.split_last() else { return };
        let Some(node) = self.list_mut(parent).filter(|l| *last < l.len()).map(|l| l.remove(*last)) else { return };
        // Taking the node out shifts the later siblings on the way to `into` one up.
        let mut into = into.to_vec();
        if into.len() > parent.len() && into.starts_with(parent) && into[parent.len()] > *last {
            into[parent.len()] -= 1;
        }
        let mut index = index;
        if into == parent && index > *last {
            index -= 1;
        }
        match self.list_mut(&into) {
            Some(list) => list.insert(index.min(list.len()), node),
            None => self.nodes.push(node),
        }
    }

    /// The project a folder belongs to: the deepest project folder it is in.
    pub fn project_of(&self, folder: &Path) -> Option<PathBuf> {
        self.projects()
            .into_iter()
            .map(|(_, p)| p)
            .filter(|p| folder.starts_with(p))
            .max_by_key(|p| p.components().count())
            .map(Path::to_path_buf)
    }
}

/// A project whose folder is gone stays in the tree, marked as missing.
pub fn missing(path: &Path) -> bool {
    !path.is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(nodes: &[Node]) -> Vec<String> {
        nodes.iter().map(Node::label).collect()
    }

    #[test]
    fn uc_01_1_a_folder_becomes_a_project_at_the_top() {
        let mut t = Tree::default();
        assert_eq!(t.add_project(Path::new("/Users/alice/code/vesta")), vec![0]);
        assert_eq!(labels(&t.nodes), ["vesta"]);
    }

    #[test]
    fn uc_01_2_a_folder_is_added_once() {
        let mut t = Tree::default();
        t.add_group("Apps");
        let at = t.add_project(Path::new("/code/vesta"));
        t.move_to(&at, &[0], 0);
        assert_eq!(t.add_project(Path::new("/code/vesta")), vec![0, 0]);
        assert_eq!(t.projects().len(), 1);
    }

    #[test]
    fn uc_01_3_projects_and_groups_go_into_groups_in_order() {
        let mut t = Tree::default();
        t.add_project(Path::new("/code/vesta"));
        t.add_project(Path::new("/code/moneta"));
        t.add_group("Apps");
        t.move_to(&[1], &[2], 0); // moneta into Apps
        t.move_to(&[0], &[1], 1); // vesta after it
        assert_eq!(labels(&t.nodes), ["Apps"]);
        let Some(Node::Group { children, .. }) = t.get(&[0]) else { panic!() };
        assert_eq!(labels(children), ["moneta", "vesta"]);
        t.add_group("Work");
        t.move_to(&[0], &[1], 0); // Apps into Work
        assert_eq!(labels(&t.nodes), ["Work"]);
        assert_eq!(t.get(&[0, 0, 1]).map(Node::label).as_deref(), Some("vesta"));
        t.move_to(&[0], &[0, 0], 0);
        assert_eq!(labels(&t.nodes), ["Work"], "a group can't go into itself");
    }

    #[test]
    fn uc_01_4_the_tree_comes_back_after_a_restart() {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("tree.json");
        let mut t = Tree::default();
        t.add_group("Work");
        t.add_project(Path::new("/code/vesta"));
        t.move_to(&[1], &[0], 0);
        t.add_project(Path::new("/code/old"));
        t.save(&file).unwrap();
        assert_eq!(Tree::load(&file), t);
    }

    #[test]
    fn uc_01_5_a_removed_group_leaves_its_contents_in_its_place() {
        let mut t = Tree::default();
        t.add_group("Work");
        t.add_group("Apps");
        t.add_project(Path::new("/code/vesta"));
        t.add_project(Path::new("/code/notes"));
        t.move_to(&[2], &[1], 0); // vesta into Apps
        t.move_to(&[1], &[0], 0); // Apps into Work
        t.move_to(&[1], &[0], 1); // notes after Apps in Work
        t.remove(&[0, 0]);
        let Some(Node::Group { children, .. }) = t.get(&[0]) else { panic!() };
        assert_eq!(labels(children), ["vesta", "notes"]);
    }

    #[test]
    fn uc_01_6_a_project_whose_folder_is_gone_is_marked_missing() {
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join("old");
        std::fs::create_dir(&old).unwrap();
        let mut t = Tree::default();
        t.add_project(&old);
        assert!(!missing(&old));
        std::fs::remove_dir(&old).unwrap();
        assert!(missing(&old));
        assert_eq!(t.projects().len(), 1, "it stays in the tree");
    }

    #[test]
    fn uc_01_7_a_renamed_group_keeps_its_place_and_contents() {
        let mut t = Tree::default();
        t.add_project(Path::new("/code/a"));
        t.add_group("Apps");
        t.add_project(Path::new("/code/vesta"));
        t.move_to(&[2], &[1], 0);
        t.rename(&[1], "Mobile");
        assert_eq!(labels(&t.nodes), ["a", "Mobile"]);
        assert_eq!(t.get(&[1, 0]).map(Node::label).as_deref(), Some("vesta"));
    }

    #[test]
    fn uc_03_5_a_folder_belongs_to_the_deepest_project_it_is_in() {
        let mut t = Tree::default();
        t.add_project(Path::new("/code"));
        t.add_project(Path::new("/code/vesta"));
        assert_eq!(t.project_of(Path::new("/code/vesta/app/src")), Some(PathBuf::from("/code/vesta")));
        assert_eq!(t.project_of(Path::new("/code/moneta")), Some(PathBuf::from("/code")));
        assert_eq!(t.project_of(Path::new("/elsewhere")), None);
    }
}
