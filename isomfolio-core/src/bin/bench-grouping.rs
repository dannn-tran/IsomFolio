//! Burst-vs-scene grouping quality benchmark.
//!
//! Answers "does the embedding-based **scene** grouper earn its keep over the
//! cheap phash **burst** grouper?" against a labelled test set, entirely inside
//! `isomfolio-core` (both groupers are pure functions; the scene descriptor is
//! model-free `gist-lite-v1`, so no inference engine or DB is involved).
//!
//! Test set = **one subfolder per ground-truth group** (the intended "shot"):
//!
//! ```text
//! testset/
//!   shot-01-portrait-recomposed/  a.jpg b.jpg c.jpg
//!   shot-02-burst/                ...
//!   _singletons/                  one-offs that should NOT group
//! ```
//!
//! Each top-level subfolder is a true group; files sitting directly in the root
//! are treated as singletons. We score each grouper against those labels with
//! pairwise Precision/Recall/F1 and the Adjusted Rand Index, swept over each
//! grouper's parameter, and count how many "recomposed" same-group pairs (far
//! apart in Hamming, so bursts can't link them visually) each one captures — the
//! direct measure of scene utility.
//!
//! Bursts are compared on the **visual signal only**: an unbounded time window
//! and capture-order = (folder, name), so frames of one shot are contiguous (the
//! realistic single-shoot ordering) and the result reflects phash, not clocks.
//!
//! Run (release — debug descriptors are far slower):
//! `cargo run --release -p isomfolio-core --bin bench-grouping -- <testset> [--limit N]`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use isomfolio_core::file_index::is_supported_extension;
use isomfolio_core::phash::{self, HashedFile};
use isomfolio_core::scene_embed::{self, SceneItem};

const UNBOUNDED_WINDOW: i64 = i64::MAX;
/// Hamming distance above which two frames are "recomposed" — well beyond any
/// useful burst threshold (default stacking is 8), so bursts can't link them by
/// similarity. The scene grouper catching these pairs is its reason to exist.
const RECOMPOSITION_HAMMING: u32 = 12;

struct Item {
    label: usize,
    hash: u64,
    embedding: Vec<f32>,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut folder: Option<PathBuf> = None;
    let mut limit: Option<usize> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--limit" => limit = args.next().and_then(|v| v.parse().ok()),
            _ if folder.is_none() => folder = Some(PathBuf::from(a)),
            _ => {}
        }
    }
    let Some(folder) = folder else {
        eprintln!("usage: bench-grouping <testset-folder> [--limit N]");
        std::process::exit(2);
    };

    // Collect (label-folder, path), ordered by (folder, name) so one shot's
    // frames are contiguous — the realistic capture order, and the fair input for
    // the sequential burst grouper.
    let mut labels: BTreeMap<String, usize> = BTreeMap::new();
    let mut files: Vec<(usize, PathBuf)> = Vec::new();
    let mut paths: Vec<(String, PathBuf)> = Vec::new();
    collect(&folder, &folder, &mut paths);
    paths.sort();
    for (label_key, path) in paths {
        let next = labels.len();
        let label = *labels.entry(label_key).or_insert(next);
        files.push((label, path));
    }
    if let Some(n) = limit {
        files.truncate(n);
    }
    if files.is_empty() {
        eprintln!("no supported images under {}", folder.display());
        std::process::exit(1);
    }

    eprint!("decoding + describing {} images… ", files.len());
    let mut items: Vec<Item> = Vec::with_capacity(files.len());
    for (label, path) in &files {
        let Ok(img) = image::open(path) else {
            eprintln!("\n  skip (decode failed): {}", path.display());
            continue;
        };
        items.push(Item {
            label: *label,
            hash: phash::dhash(&img),
            embedding: scene_embed::scene_embedding(&img),
        });
    }
    eprintln!("done");

    let truth: Vec<usize> = items.iter().map(|i| i.label).collect();
    let n_groups = labels.len();
    let true_singletons = count_singletons(&truth);

    println!("\ntestset: {}", folder.display());
    println!(
        "{} images · {} ground-truth groups ({} are singletons) · visual signal only (unbounded time window)\n",
        items.len(),
        n_groups,
        true_singletons
    );

    // — Bursts: sequential phash grouping over a Hamming sweep —
    let hashed: Vec<HashedFile> =
        items.iter().enumerate().map(|(i, it)| HashedFile { hash: it.hash, time: i as i64 }).collect();
    println!("BURSTS — phash dhash, capture-order, unbounded window");
    println!("  thr  groups  prec   rec    F1     ARI");
    let mut best_burst = Best::default();
    for thr in 0..=16u32 {
        let groups = phash::group_stacks(&hashed, thr, UNBOUNDED_WINDOW);
        let pred = labels_from_groups(&groups, items.len());
        let s = score(&truth, &pred);
        println!("  {thr:>3}  {:>6}  {:>5.3}  {:>5.3}  {:>5.3}  {:>5.3}", groups.len(), s.precision, s.recall, s.f1, s.ari);
        best_burst.consider(s, thr as f32, 0, pred);
    }
    println!("  → best F1 {:.3} @ thr={}\n", best_burst.f1, best_burst.param as u32);

    // — Scenes: whitened gist descriptors, DBSCAN over an eps × min_pts grid —
    let raw: Vec<Vec<f32>> = items.iter().map(|i| i.embedding.clone()).collect();
    let whitened = scene_embed::whiten(&raw);
    let scene_items: Vec<SceneItem> =
        whitened.iter().map(|e| SceneItem { embedding: e.clone(), sharpness: 0.0 }).collect();
    println!("SCENES — gist-lite, whitened, DBSCAN (cosine)");
    println!("  eps   mp  groups  prec   rec    F1     ARI");
    let mut best_scene = Best::default();
    for &mp in &[1usize, 2] {
        for &eps in &[0.05f32, 0.10, 0.15, 0.20, 0.25, 0.30, 0.40, 0.50, 0.60] {
            let groups = scene_embed::group_scenes(&scene_items, eps, mp);
            let pred = labels_from_groups(&groups, items.len());
            let s = score(&truth, &pred);
            println!("  {eps:>4.2}  {mp:>1}  {:>6}  {:>5.3}  {:>5.3}  {:>5.3}  {:>5.3}", groups.len(), s.precision, s.recall, s.f1, s.ari);
            best_scene.consider(s, eps, mp, pred);
        }
    }
    println!("  → best F1 {:.3} @ eps={:.2}, min_pts={}\n", best_scene.f1, best_scene.param, best_scene.min_pts);

    // — Utility: recomposed same-group pairs bursts can't link by similarity —
    let recomp = recomposition_pairs(&items);
    let burst_caught = pairs_together(&recomp, &best_burst.pred);
    let scene_caught = pairs_together(&recomp, &best_scene.pred);
    println!("RECOMPOSITIONS — same-group pairs with Hamming > {RECOMPOSITION_HAMMING}: {}", recomp.len());
    if !recomp.is_empty() {
        println!("  captured by best-F1 bursts: {burst_caught} ({:.0}%)", pct(burst_caught, recomp.len()));
        println!("  captured by best-F1 scenes: {scene_caught} ({:.0}%)", pct(scene_caught, recomp.len()));
    }

    println!("\nVERDICT");
    let df1 = best_scene.f1 - best_burst.f1;
    println!("  scene F1 {} burst F1 by {:+.3}", if df1 >= 0.0 { "beats" } else { "trails" }, df1);
    if !recomp.is_empty() {
        println!("  scenes capture {scene_caught}/{} recompositions bursts can't reach by phash", recomp.len());
    } else {
        println!("  no recomposed pairs in this set — it can't exercise the scene grouper's advantage");
    }
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if path.extension().and_then(|x| x.to_str()).map(is_supported_extension).unwrap_or(false) {
            out.push((label_of(root, &path), path));
        }
    }
}

/// Ground-truth label = the immediate subfolder of the test-set root that the
/// file lives under; files directly in the root get a unique per-file label
/// (true singletons).
fn label_of(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    match rel.components().next() {
        Some(first) if rel.components().count() > 1 => first.as_os_str().to_string_lossy().into_owned(),
        _ => format!("__single__{}", path.display()),
    }
}

#[derive(Default)]
struct Score {
    precision: f64,
    recall: f64,
    f1: f64,
    ari: f64,
}

#[derive(Default)]
struct Best {
    f1: f64,
    param: f32,
    min_pts: usize,
    pred: Vec<usize>,
}

impl Best {
    fn consider(&mut self, s: Score, param: f32, min_pts: usize, pred: Vec<usize>) {
        if pred.is_empty() {
            return;
        }
        if s.f1 > self.f1 || self.pred.is_empty() {
            self.f1 = s.f1;
            self.param = param;
            self.min_pts = min_pts;
            self.pred = pred;
        }
    }
}

/// Turn grouper output (vecs of indices, ≥2 each, singletons dropped) into a flat
/// per-item cluster label; every ungrouped item becomes its own singleton cluster
/// so pairwise scoring treats "not grouped" correctly.
fn labels_from_groups(groups: &[Vec<usize>], n: usize) -> Vec<usize> {
    let mut labels = vec![0usize; n];
    let mut next = 0usize;
    let mut assigned = vec![false; n];
    for g in groups {
        let id = next;
        next += 1;
        for &i in g {
            labels[i] = id;
            assigned[i] = true;
        }
    }
    for (i, a) in assigned.iter().enumerate() {
        if !*a {
            labels[i] = next;
            next += 1;
        }
    }
    labels
}

/// Pairwise Precision/Recall/F1 + Adjusted Rand Index of a predicted clustering
/// against the ground truth, both as flat per-item labels.
fn score(truth: &[usize], pred: &[usize]) -> Score {
    let n = truth.len();
    let (mut tp, mut fp, mut fn_, mut tn) = (0u64, 0u64, 0u64, 0u64);
    for i in 0..n {
        for j in (i + 1)..n {
            let same_t = truth[i] == truth[j];
            let same_p = pred[i] == pred[j];
            match (same_t, same_p) {
                (true, true) => tp += 1,
                (false, true) => fp += 1,
                (true, false) => fn_ += 1,
                (false, false) => tn += 1,
            }
        }
    }
    let precision = ratio(tp, tp + fp);
    let recall = ratio(tp, tp + fn_);
    let f1 = if precision + recall > 0.0 { 2.0 * precision * recall / (precision + recall) } else { 0.0 };
    Score { precision, recall, f1, ari: adjusted_rand(tp, fp, fn_, tn) }
}

/// Adjusted Rand Index from the pair-confusion counts (Hubert–Arabie form).
fn adjusted_rand(tp: u64, fp: u64, fn_: u64, tn: u64) -> f64 {
    let (tp, fp, fn_, tn) = (tp as f64, fp as f64, fn_ as f64, tn as f64);
    let total = tp + fp + fn_ + tn;
    if total == 0.0 {
        return 0.0;
    }
    let index = tp;
    let expected = (tp + fp) * (tp + fn_) / total;
    let max = 0.5 * ((tp + fp) + (tp + fn_));
    if (max - expected).abs() < f64::EPSILON {
        return 0.0;
    }
    (index - expected) / (max - expected)
}

fn recomposition_pairs(items: &[Item]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for i in 0..items.len() {
        for j in (i + 1)..items.len() {
            if items[i].label == items[j].label
                && (items[i].hash ^ items[j].hash).count_ones() > RECOMPOSITION_HAMMING
            {
                out.push((i, j));
            }
        }
    }
    out
}

fn pairs_together(pairs: &[(usize, usize)], pred: &[usize]) -> usize {
    if pred.is_empty() {
        return 0;
    }
    pairs.iter().filter(|&&(i, j)| pred[i] == pred[j]).count()
}

fn count_singletons(truth: &[usize]) -> usize {
    let mut counts: BTreeMap<usize, usize> = BTreeMap::new();
    for &l in truth {
        *counts.entry(l).or_default() += 1;
    }
    counts.values().filter(|&&c| c == 1).count()
}

fn ratio(num: u64, den: u64) -> f64 {
    if den == 0 { 0.0 } else { num as f64 / den as f64 }
}

fn pct(num: usize, den: usize) -> f64 {
    if den == 0 { 0.0 } else { 100.0 * num as f64 / den as f64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod labels_from_groups {
        use super::*;

        #[test]
        fn ungrouped_items_become_distinct_singletons() {
            // items 0,1 grouped; 2,3 ungrouped → three distinct cluster ids.
            let labels = labels_from_groups(&[vec![0, 1]], 4);
            assert_eq!(labels[0], labels[1]);
            assert_ne!(labels[0], labels[2]);
            assert_ne!(labels[2], labels[3]);
        }

        #[test]
        fn two_groups_get_separate_ids() {
            let labels = labels_from_groups(&[vec![0, 1], vec![2, 3]], 4);
            assert_eq!(labels[0], labels[1]);
            assert_eq!(labels[2], labels[3]);
            assert_ne!(labels[0], labels[2]);
        }
    }

    mod score {
        use super::*;

        #[test]
        fn perfect_clustering_scores_one() {
            let truth = vec![0, 0, 1, 1];
            let pred = vec![5, 5, 9, 9]; // same partition, different ids
            let s = score(&truth, &pred);
            assert!((s.f1 - 1.0).abs() < 1e-9);
            assert!((s.precision - 1.0).abs() < 1e-9);
            assert!((s.recall - 1.0).abs() < 1e-9);
            assert!((s.ari - 1.0).abs() < 1e-9);
        }

        #[test]
        fn everything_split_has_zero_recall() {
            let truth = vec![0, 0, 1, 1];
            let pred = vec![0, 1, 2, 3]; // all singletons
            let s = score(&truth, &pred);
            assert_eq!(s.recall, 0.0);
            assert_eq!(s.f1, 0.0);
        }

        #[test]
        fn over_merging_costs_precision() {
            let truth = vec![0, 0, 1, 1];
            let pred = vec![0, 0, 0, 0]; // one mega-cluster
            let s = score(&truth, &pred);
            assert!((s.recall - 1.0).abs() < 1e-9, "all true pairs are together");
            assert!(s.precision < 1.0, "but cross-group pairs are false positives");
        }

        #[test]
        fn ari_is_zero_for_chance_like_grouping() {
            // Random independent labels should sit near 0 (here, exactly the
            // expected-index case): one true pair, predicted into different cells.
            let truth = vec![0, 0, 1, 1];
            let pred = vec![0, 1, 0, 1];
            let s = score(&truth, &pred);
            assert!(s.ari.abs() < 0.5);
        }
    }

    mod recomposition {
        use super::*;

        fn item(label: usize, hash: u64) -> Item {
            Item { label, hash, embedding: vec![] }
        }

        #[test]
        fn flags_far_apart_same_group_pairs_only() {
            let items = vec![
                item(0, 0b0),
                item(0, (1u64 << (RECOMPOSITION_HAMMING + 1)) - 1), // many bits set, same group
                item(1, 0b0),                                       // different group, far — ignored
            ];
            let pairs = recomposition_pairs(&items);
            assert_eq!(pairs, vec![(0, 1)]);
        }

        #[test]
        fn near_duplicates_are_not_recompositions() {
            let items = vec![item(0, 0b0), item(0, 0b1)]; // Hamming 1
            assert!(recomposition_pairs(&items).is_empty());
        }
    }
}
