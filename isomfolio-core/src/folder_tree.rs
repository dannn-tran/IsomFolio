use std::collections::BTreeMap;
use std::path::MAIN_SEPARATOR;

/// A folder in the sidebar tree. Built from the flat list of leaf folders that
/// contain photos (`get_folder_counts`), reconstructing the intermediate
/// ancestors so the sidebar can render a navigable hierarchy.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderNode {
    /// Full normalised path of this folder.
    pub path: String,
    /// Display name (basename).
    pub name: String,
    /// Photos directly in this folder (excludes descendants).
    pub direct_count: usize,
    /// Photos in this folder plus all descendants.
    pub total_count: usize,
    pub children: Vec<FolderNode>,
}

#[derive(Default)]
struct Trie {
    count: usize,
    children: BTreeMap<String, Trie>,
}

/// Build the sidebar folder forest from `(folder_path, direct_count)` pairs.
///
/// Pure pass-through ancestors (a single child, no files of their own) are
/// collapsed so the displayed roots are the deepest common folders the user
/// actually has photos under — never `/`, `/Users`, etc.
pub fn build_tree(folders: &[(String, usize)]) -> Vec<FolderNode> {
    build_tree_sep(folders, MAIN_SEPARATOR)
}

fn build_tree_sep(folders: &[(String, usize)], sep: char) -> Vec<FolderNode> {
    let mut root = Trie::default();
    for (path, count) in folders {
        let trimmed = path.trim_end_matches(sep);
        if trimmed.is_empty() {
            continue;
        }
        let mut node = &mut root;
        for comp in trimmed.split(sep) {
            node = node.children.entry(comp.to_string()).or_default();
        }
        node.count += *count;
    }

    root.children
        .iter()
        .flat_map(|(comp, child)| collapse(to_node(comp, comp, child, sep)))
        .collect()
}

fn join(prefix: &str, comp: &str, sep: char) -> String {
    format!("{prefix}{sep}{comp}")
}

fn to_node(path: &str, name: &str, t: &Trie, sep: char) -> FolderNode {
    let mut children: Vec<FolderNode> = t
        .children
        .iter()
        .map(|(comp, child)| to_node(&join(path, comp, sep), comp, child, sep))
        .collect();
    children.sort_by(|a, b| a.name.cmp(&b.name));
    let total = t.count + children.iter().map(|c| c.total_count).sum::<usize>();
    FolderNode {
        path: path.to_string(),
        name: name.to_string(),
        direct_count: t.count,
        total_count: total,
        children,
    }
}

/// Descend through pass-through ancestors (no own files, exactly one child)
/// so the returned root is the first folder that branches or holds photos.
fn collapse(mut node: FolderNode) -> Vec<FolderNode> {
    while node.direct_count == 0 && node.children.len() == 1 {
        node = node.children.into_iter().next().unwrap();
    }
    vec![node]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(folders: &[(&str, usize)]) -> Vec<FolderNode> {
        let owned: Vec<(String, usize)> =
            folders.iter().map(|(p, c)| (p.to_string(), *c)).collect();
        build_tree_sep(&owned, '/')
    }

    #[test]
    fn empty_input_yields_empty_forest() {
        assert!(build(&[]).is_empty());
    }

    #[test]
    fn single_leaf_collapses_to_one_root() {
        let tree = build(&[("/users/me/photos", 3)]);
        assert_eq!(tree.len(), 1);
        let root = &tree[0];
        assert_eq!(root.path, "/users/me/photos");
        assert_eq!(root.name, "photos");
        assert_eq!(root.direct_count, 3);
        assert_eq!(root.total_count, 3);
        assert!(root.children.is_empty());
    }

    #[test]
    fn nested_subfolders_become_children() {
        let tree = build(&[
            ("/photos/2011", 10),
            ("/photos/2018", 5),
        ]);
        assert_eq!(tree.len(), 1);
        let root = &tree[0];
        assert_eq!(root.name, "photos");
        assert_eq!(root.direct_count, 0);
        assert_eq!(root.total_count, 15);
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].name, "2011");
        assert_eq!(root.children[0].total_count, 10);
        assert_eq!(root.children[1].name, "2018");
    }

    #[test]
    fn parent_with_own_and_descendant_photos() {
        let tree = build(&[
            ("/photos", 4),
            ("/photos/2011", 10),
        ]);
        let root = &tree[0];
        assert_eq!(root.name, "photos");
        assert_eq!(root.direct_count, 4);
        assert_eq!(root.total_count, 14);
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].name, "2011");
    }

    #[test]
    fn deep_passthrough_chain_collapses_to_branch_point() {
        let tree = build(&[
            ("/a/b/c/x", 1),
            ("/a/b/c/y", 2),
        ]);
        assert_eq!(tree.len(), 1);
        let root = &tree[0];
        assert_eq!(root.path, "/a/b/c");
        assert_eq!(root.name, "c");
        assert_eq!(root.children.len(), 2);
    }

    #[test]
    fn grandchildren_nest_correctly() {
        let tree = build(&[
            ("/lib/2018/wedding", 30),
            ("/lib/2018/holiday", 12),
            ("/lib/2019", 7),
        ]);
        let root = &tree[0];
        assert_eq!(root.name, "lib");
        assert_eq!(root.total_count, 49);
        assert_eq!(root.children.len(), 2);
        let y2018 = &root.children[0];
        assert_eq!(y2018.name, "2018");
        assert_eq!(y2018.total_count, 42);
        assert_eq!(y2018.children.len(), 2);
        // children sorted alphabetically: holiday before wedding
        assert_eq!(y2018.children[0].name, "holiday");
        assert_eq!(y2018.children[1].name, "wedding");
    }

    #[test]
    fn trailing_separator_is_ignored() {
        let tree = build(&[("/photos/2011/", 5)]);
        assert_eq!(tree[0].path, "/photos/2011");
        assert_eq!(tree[0].total_count, 5);
    }
}
