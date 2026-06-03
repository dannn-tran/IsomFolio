use std::collections::BTreeMap;
use std::path::MAIN_SEPARATOR;

/// One folder segment within a row's breadcrumb: its display name and the
/// normalised path it navigates to when clicked.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderSeg {
    pub name: String,
    pub path: String,
}

/// A folder in the sidebar tree. Built from the flat list of leaf folders that
/// contain photos (`get_folder_counts`), reconstructing the intermediate
/// ancestors so the sidebar can render a navigable hierarchy.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderNode {
    /// Full normalised path of this folder (the deepest segment of `chain`).
    pub path: String,
    /// Display name (basename) of the deepest segment.
    pub name: String,
    /// Breadcrumb shown on this row: a run of single-child pass-through
    /// ancestors compacted together (VS Code-style "compact folders"), ending
    /// at this node. Length 1 for an ordinary folder; >1 when the chain has no
    /// branching and no photos until the last segment. Each segment is
    /// separately clickable.
    pub chain: Vec<FolderSeg>,
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
/// triples plus the set of **library root** key-paths (the folders the user
/// added). The key is the case-folded path (trie structure + node `path`); the
/// display path carries real-case segment names for each `FolderNode.name`.
///
/// The forest is rooted at the library anchors — the common ancestor of the
/// added folders on each drive (see [`library_anchors`]) — so the breadcrumb
/// starts at the user's content, not the filesystem root: the noisy `/Users/me`
/// prefix above it is hidden. Below an anchor, single-child pass-through runs are
/// compacted into one breadcrumb row. When `roots` is empty (e.g. unit tests),
/// it falls back to filesystem-top roots.
pub fn build_tree(folders: &[(String, String, usize)], roots: &[String]) -> Vec<FolderNode> {
    build_tree_sep(folders, roots, MAIN_SEPARATOR)
}

fn build_tree_sep(
    folders: &[(String, String, usize)],
    roots: &[String],
    sep: char,
) -> Vec<FolderNode> {
    // An absolute path's leading separator yields an empty first segment
    // (`/photos` → `["", "photos"]`). Drop empty segments entirely — the root
    // "/" is not a real folder — and re-attach the leading separator once when
    // building top-level paths. Divergent top-level dirs (e.g. `/Users/...` and
    // `/Volumes/...`) then naturally form sibling roots, a forest, with no
    // nameless "ghost" parent.
    let absolute = folders.iter().any(|(p, _, _)| p.starts_with(sep));
    let mut root = Trie::default();
    for (path, display, count) in folders {
        let mut disp_segs = display.split(sep).filter(|s| !s.is_empty());
        let mut node = &mut root;
        let mut any = false;
        for comp in path.split(sep).filter(|s| !s.is_empty()) {
            any = true;
            // Pair each key segment with its display counterpart; the two paths
            // share structure (both canonicalised) so they align 1:1.
            let disp = disp_segs.next();
            node = node.children.entry(comp.to_string()).or_default();
            if node.display.is_empty() {
                node.display = disp.unwrap_or(comp).to_string();
            }
        }
        if any {
            node.count += *count;
        }
    }

    let anchors = library_anchors(roots, sep);
    let mut nodes: Vec<FolderNode> = if anchors.is_empty() {
        // Fallback: roots unknown — display from the filesystem-top dirs.
        let prefix = if absolute { sep.to_string() } else { String::new() };
        root.children
            .iter()
            .map(|(comp, child)| {
                let path = format!("{prefix}{comp}");
                compact(to_node(&path, &child.display, comp, child, sep))
            })
            .collect()
    } else {
        anchors
            .iter()
            .filter_map(|anchor| {
                let node = node_at(&root, anchor, sep)?;
                let last = anchor.rsplit(sep).find(|s| !s.is_empty()).unwrap_or(anchor);
                Some(compact(to_node(anchor, &node.display, last, node, sep)))
            })
            .collect()
    };
    nodes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    nodes
}

/// Walk the trie to the node at `path` (segments are case-folded keys).
fn node_at<'a>(root: &'a Trie, path: &str, sep: char) -> Option<&'a Trie> {
    let mut node = root;
    for comp in path.split(sep).filter(|s| !s.is_empty()) {
        node = node.children.get(comp)?;
    }
    Some(node)
}

/// Deepest common ancestor of `paths`, segment-wise, as an absolute path
/// (empty when they share no leading segment). A single path is its own ancestor.
fn common_ancestor(paths: &[&str], sep: char) -> String {
    let segs = |p: &str| -> Vec<String> {
        p.split(sep).filter(|s| !s.is_empty()).map(str::to_string).collect()
    };
    let mut common = match paths.first() {
        Some(p) => segs(p),
        None => return String::new(),
    };
    for p in &paths[1..] {
        let other = segs(p);
        let n = common.iter().zip(&other).take_while(|(a, b)| a == b).count();
        common.truncate(n);
    }
    if common.is_empty() {
        String::new()
    } else {
        format!("{sep}{}", common.join(&sep.to_string()))
    }
}

/// The forest anchors: the library root(s) the breadcrumb should start at. All
/// added folders normally share a common ancestor (one anchor); when they span
/// drives with no shared prefix, they're grouped by their top-level segment so
/// each drive gets its own anchor instead of collapsing to a bare `/`.
fn library_anchors(roots: &[String], sep: char) -> Vec<String> {
    if roots.is_empty() {
        return Vec::new();
    }
    let refs: Vec<&str> = roots.iter().map(String::as_str).collect();
    let ca = common_ancestor(&refs, sep);
    if !ca.is_empty() {
        return vec![ca];
    }
    let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for r in &refs {
        let top = r.split(sep).find(|s| !s.is_empty()).unwrap_or("");
        groups.entry(top).or_default().push(r);
    }
    groups
        .values()
        .map(|g| common_ancestor(g, sep))
        .filter(|a| !a.is_empty())
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
        chain: vec![FolderSeg { name: name.to_string(), path: path.to_string() }],
        direct_count: t.count,
        total_count: total,
        children,
    }
}

/// Compact a chain of pass-through folders (no own photos, exactly one child)
/// into a single breadcrumb row, then recurse into the branch's children — the
/// VS Code "compact folders" model. Names stay visible (each segment in `chain`
/// is separately clickable) instead of being collapsed away.
fn compact(mut node: FolderNode) -> FolderNode {
    while node.direct_count == 0 && node.children.len() == 1 {
        let mut child = node.children.remove(0);
        node.chain.append(&mut child.chain);
        node.path = child.path;
        node.name = child.name;
        node.direct_count = child.direct_count;
        node.children = child.children;
        // total_count is unchanged: this node had no photos of its own, so its
        // total already equalled the absorbed child's total.
    }
    node.children = node.children.into_iter().map(compact).collect();
    node
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
        // No roots → filesystem-top fallback (exercises the trie/compaction).
        build_tree_sep(&owned, &[], '/')
    }

    fn build_anchored(folders: &[(&str, usize)], roots: &[&str]) -> Vec<FolderNode> {
        let owned: Vec<(String, String, usize)> =
            folders.iter().map(|(p, c)| (p.to_string(), p.to_string(), *c)).collect();
        let roots: Vec<String> = roots.iter().map(|s| s.to_string()).collect();
        build_tree_sep(&owned, &roots, '/')
    }

    #[test]
    fn display_path_supplies_real_case_names() {
        // Two leaves share /Lib/2018, so it survives as a branch-point ancestor.
        let tree = build_tree_sep(
            &[
                ("/lib/2018/wedding".into(), "/Lib/2018/Wedding".into(), 3),
                ("/lib/2018/holiday".into(), "/Lib/2018/Holiday".into(), 2),
            ],
            &[],
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
        let tree = build_tree_sep(&[("/photos/2011".into(), String::new(), 5)], &[], '/');
        assert_eq!(tree[0].name, "2011");
    }

    #[test]
    fn single_child_chain_compacts_into_one_breadcrumb_row() {
        let tree = build(&[("/a/b/c", 3)]);
        assert_eq!(tree.len(), 1);
        let row = &tree[0];
        // One row, but the breadcrumb keeps every pass-through segment, each
        // with its own navigable path.
        let chain: Vec<(&str, &str)> =
            row.chain.iter().map(|s| (s.name.as_str(), s.path.as_str())).collect();
        assert_eq!(chain, vec![("a", "/a"), ("b", "/a/b"), ("c", "/a/b/c")]);
        assert_eq!(row.path, "/a/b/c");
        assert_eq!(row.name, "c");
    }

    #[test]
    fn branching_stops_compaction() {
        // /a has two children, so it is its own row (chain == [a]); the children
        // are separate single-segment rows.
        let tree = build(&[("/a/b", 1), ("/a/c", 1)]);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].chain.len(), 1);
        assert_eq!(tree[0].name, "a");
        assert_eq!(tree[0].children.len(), 2);
        assert!(tree[0].children.iter().all(|c| c.chain.len() == 1));
    }

    #[test]
    fn anchor_starts_breadcrumb_at_the_library_root() {
        // Added /users/me/photos; its filesystem prefix is hidden, so the root
        // row begins at "photos" rather than "users / me / photos".
        let tree = build_anchored(
            &[("/users/me/photos/2024", 3)],
            &["/users/me/photos"],
        );
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].chain.first().unwrap().name, "photos");
        assert_eq!(tree[0].chain.first().unwrap().path, "/users/me/photos");
    }

    #[test]
    fn two_roots_on_same_drive_anchor_at_common_ancestor() {
        let tree = build_anchored(
            &[("/users/me/photos", 3), ("/users/me/pics", 2)],
            &["/users/me/photos", "/users/me/pics"],
        );
        // Common ancestor /users/me becomes the single (virtual) root.
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].path, "/users/me");
        let names: Vec<&str> = tree[0].children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["photos", "pics"]);
    }

    #[test]
    fn roots_across_drives_anchor_separately() {
        let tree = build_anchored(
            &[("/users/me/photos", 3), ("/volumes/sd/dcim", 2)],
            &["/users/me/photos", "/volumes/sd/dcim"],
        );
        assert_eq!(tree.len(), 2);
        let paths: Vec<&str> = tree.iter().map(|n| n.path.as_str()).collect();
        assert!(paths.contains(&"/users/me/photos") && paths.contains(&"/volumes/sd/dcim"));
    }

    #[test]
    fn divergent_top_level_roots_have_no_ghost_parent() {
        // Different first-level dirs (e.g. internal vs external volume) share only
        // the leading "/" — the empty leading segment must not surface as a root.
        let tree = build(&[("/users/me/photos", 3), ("/mnt/sd/dcim", 2)]);
        assert_eq!(tree.len(), 2);
        assert!(tree.iter().all(|n| !n.name.is_empty() && !n.path.is_empty()));
        let names: Vec<&str> = tree.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"photos") && names.contains(&"dcim"));
        assert_eq!(tree.iter().find(|n| n.name == "photos").unwrap().path, "/users/me/photos");
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
