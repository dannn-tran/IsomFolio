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
    /// Real-case segment name for this node (display). Empty until a path sets it.
    display: String,
    children: BTreeMap<String, Trie>,
}

/// Build the sidebar folder forest from `(folder_key, folder_display, direct_count)`
/// triples. The key is the case-folded path (trie structure + node `path`); the
/// display path carries real-case segment names for each `FolderNode.name`.
///
/// Pure pass-through ancestors (a single child, no files of their own) are
/// collapsed so the displayed roots are the deepest common folders the user
/// actually has photos under — never `/`, `/Users`, etc.
pub fn build_tree(folders: &[(String, String, usize)]) -> Vec<FolderNode> {
    build_tree_sep(folders, MAIN_SEPARATOR)
}

fn build_tree_sep(folders: &[(String, String, usize)], sep: char) -> Vec<FolderNode> {
    let mut root = Trie::default();
    for (path, display, count) in folders {
        let trimmed = path.trim_end_matches(sep);
        if trimmed.is_empty() {
            continue;
        }
        let mut disp_segs = display.trim_end_matches(sep).split(sep);
        let mut node = &mut root;
        for comp in trimmed.split(sep) {
            // Pair each key segment with its display counterpart; the two paths
            // share structure (both canonicalised) so they align 1:1.
            let disp = disp_segs.next().filter(|s| !s.is_empty());
            node = node.children.entry(comp.to_string()).or_default();
            if node.display.is_empty() {
                node.display = disp.unwrap_or(comp).to_string();
            }
        }
        node.count += *count;
    }

    root.children
        .iter()
        .flat_map(|(comp, child)| collapse(to_node(comp, &child.display, comp, child, sep)))
        .collect()
}

fn join(prefix: &str, comp: &str, sep: char) -> String {
    format!("{prefix}{sep}{comp}")
}

fn to_node(path: &str, display: &str, key_comp: &str, t: &Trie, sep: char) -> FolderNode {
    let name = if display.is_empty() { key_comp } else { display };
    let mut children: Vec<FolderNode> = t
        .children
        .iter()
        .map(|(comp, child)| to_node(&join(path, comp, sep), &child.display, comp, child, sep))
        .collect();
    children.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
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

/// Folder paths to expand so `target` and its immediate children are visible:
/// every ancestor of `target`, `target` itself, and any descendant that has
/// children. Leaves (nothing to expand) are never returned. Used to reveal a
/// freshly-synced folder's subtree in the sidebar instead of leaving it collapsed.
pub fn expand_paths_for(tree: &[FolderNode], target: &str) -> Vec<String> {
    expand_paths_for_sep(tree, target, MAIN_SEPARATOR)
}

fn expand_paths_for_sep(tree: &[FolderNode], target: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    for node in tree {
        collect_expand(node, target, sep, &mut out);
    }
    out
}

fn collect_expand(node: &FolderNode, target: &str, sep: char, out: &mut Vec<String>) {
    let is_target_or_desc =
        node.path == target || node.path.starts_with(&format!("{target}{sep}"));
    let is_ancestor = target.starts_with(&format!("{}{sep}", node.path));
    if !node.children.is_empty() && (is_target_or_desc || is_ancestor) {
        out.push(node.path.clone());
    }
    for c in &node.children {
        collect_expand(c, target, sep, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(folders: &[(&str, usize)]) -> Vec<FolderNode> {
        // Display path == key path here, so node names equal the key segments.
        let owned: Vec<(String, String, usize)> =
            folders.iter().map(|(p, c)| (p.to_string(), p.to_string(), *c)).collect();
        build_tree_sep(&owned, '/')
    }

    #[test]
    fn display_path_supplies_real_case_names() {
        // Two leaves share /Lib/2018, so it survives as a branch-point ancestor.
        let tree = build_tree_sep(
            &[
                ("/lib/2018/wedding".into(), "/Lib/2018/Wedding".into(), 3),
                ("/lib/2018/holiday".into(), "/Lib/2018/Holiday".into(), 2),
            ],
            '/',
        );
        // path stays the case-folded key; name comes from the display path.
        assert_eq!(tree[0].path, "/lib/2018");
        assert_eq!(tree[0].name, "2018");
        assert_eq!(tree[0].children[0].path, "/lib/2018/holiday");
        assert_eq!(tree[0].children[0].name, "Holiday");
        assert_eq!(tree[0].children[1].name, "Wedding");
    }

    #[test]
    fn empty_display_falls_back_to_key_segment() {
        let tree = build_tree_sep(&[("/photos/2011".into(), String::new(), 5)], '/');
        assert_eq!(tree[0].name, "2011");
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
    fn zero_count_root_still_appears_as_leaf() {
        // A freshly-added library root with no indexed files yet (count 0)
        // must still surface as a navigable leaf, not be collapsed away.
        let tree = build(&[("/users/me/photos", 0)]);
        assert_eq!(tree.len(), 1);
        let root = &tree[0];
        assert_eq!(root.path, "/users/me/photos");
        assert_eq!(root.name, "photos");
        assert_eq!(root.direct_count, 0);
        assert_eq!(root.total_count, 0);
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

    mod expand_paths_for {
        use super::*;

        fn expand(folders: &[(&str, usize)], target: &str) -> Vec<String> {
            let tree = build(folders);
            super::super::expand_paths_for_sep(&tree, target, '/')
        }

        #[test]
        fn parent_with_children_expands_parent_to_reveal_them() {
            let got = expand(&[("/lib/child1", 3), ("/lib/child2", 2)], "/lib");
            assert_eq!(got, vec!["/lib".to_string()]);
        }

        #[test]
        fn collapsed_branch_point_below_target_is_expanded() {
            // sync /a, but the displayed root collapses to /a/b/c
            let got = expand(&[("/a/b/c/x", 1), ("/a/b/c/y", 2)], "/a");
            assert_eq!(got, vec!["/a/b/c".to_string()]);
        }

        #[test]
        fn ancestors_of_target_are_expanded_so_target_is_visible() {
            let mut got = expand(
                &[("/lib/2018/wedding", 30), ("/lib/2018/holiday", 12), ("/lib/2019", 7)],
                "/lib/2018",
            );
            got.sort();
            assert_eq!(got, vec!["/lib".to_string(), "/lib/2018".to_string()]);
        }

        #[test]
        fn leaf_target_yields_no_expansion() {
            let got = expand(&[("/lib/child1", 3), ("/lib/child2", 2)], "/lib/child1");
            assert_eq!(got, vec!["/lib".to_string()]);
        }
    }
}
