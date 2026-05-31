//! Face embedding clustering. Ported from the C# Faces extension's `Clustering`
//! class; the inference engine returns raw embeddings and the host clusters them.

use sha2::{Digest, Sha256};

use crate::models::{FaceClusterMember, FaceEmbeddingRow};

/// Cluster id for faces that DBSCAN (or centroid assignment) leaves unclustered.
pub const NOISE_CLUSTER_ID: &str = "face-unknown";

/// Result of turning per-face labels into persistable clusters.
pub struct ClusterAssembly {
    /// Clustered faces plus noise (under [`NOISE_CLUSTER_ID`]).
    pub members: Vec<FaceClusterMember>,
    /// (cluster_id, centroid) for real clusters only — noise has no centroid.
    pub centroids: Vec<(String, Vec<f32>)>,
}

/// Group labelled faces into clusters: real clusters get a content-derived
/// stable id and an L2-normalised centroid; label `< 0` faces go to
/// [`NOISE_CLUSTER_ID`]. `rows` and `labels` must align by index.
pub fn assemble_clusters(rows: &[FaceEmbeddingRow], labels: &[i32]) -> ClusterAssembly {
    use std::collections::BTreeMap;

    let mut by_label: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
    let mut noise: Vec<usize> = Vec::new();
    for (i, &label) in labels.iter().enumerate() {
        if label < 0 {
            noise.push(i);
        } else {
            by_label.entry(label).or_default().push(i);
        }
    }

    // Largest clusters first — purely cosmetic, but keeps output deterministic.
    let mut groups: Vec<Vec<usize>> = by_label.into_values().collect();
    groups.sort_by(|a, b| b.len().cmp(&a.len()));

    let mut members = Vec::new();
    let mut centroids = Vec::new();

    for group in groups {
        let id = stable_cluster_id(group.iter().map(|&i| {
            (rows[i].file_id.as_str(), rows[i].bbox_x, rows[i].bbox_y)
        }));
        let vecs: Vec<Vec<f32>> = group.iter().map(|&i| rows[i].vec.clone()).collect();
        centroids.push((id.clone(), compute_centroid(&vecs)));
        for &i in &group {
            let r = &rows[i];
            members.push(FaceClusterMember {
                cluster_id: id.clone(),
                file_id: r.file_id.clone(),
                bbox_x: r.bbox_x,
                bbox_y: r.bbox_y,
                bbox_w: r.bbox_w,
                bbox_h: r.bbox_h,
            });
        }
    }

    for &i in &noise {
        let r = &rows[i];
        members.push(FaceClusterMember {
            cluster_id: NOISE_CLUSTER_ID.to_string(),
            file_id: r.file_id.clone(),
            bbox_x: r.bbox_x,
            bbox_y: r.bbox_y,
            bbox_w: r.bbox_w,
            bbox_h: r.bbox_h,
        });
    }

    ClusterAssembly { members, centroids }
}

/// Content-derived cluster id, stable across runs for the same membership.
/// Mirrors the C# `StableClusterId`: sorted `file_id:x.x:y.y` keys, SHA-256,
/// first 16 lowercase hex chars.
fn stable_cluster_id<'a>(members: impl Iterator<Item = (&'a str, f64, f64)>) -> String {
    let mut keys: Vec<String> = members
        .map(|(file_id, x, y)| format!("{file_id}:{x:.1}:{y:.1}"))
        .collect();
    keys.sort();
    let combined = keys.join("\n");

    let digest = Sha256::digest(combined.as_bytes());
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("face-{hex}")
}

/// DBSCAN over face embeddings using cosine distance. Returns a label per
/// embedding; `-1` marks noise. `eps` is a cosine-distance radius, so two
/// embeddings are neighbours when their cosine similarity is `>= 1 - eps`.
pub fn dbscan(embeddings: &[Vec<f32>], eps: f32, min_pts: usize) -> Vec<i32> {
    let n = embeddings.len();
    let mut labels = vec![-1i32; n];
    let neighbors = precompute_neighbors(embeddings, eps);

    let mut cluster_id = 0i32;
    for i in 0..n {
        if labels[i] != -1 {
            continue;
        }
        if neighbors[i].len() < min_pts {
            continue;
        }

        labels[i] = cluster_id;
        let mut queue: std::collections::VecDeque<usize> = neighbors[i].iter().copied().collect();

        while let Some(j) = queue.pop_front() {
            if labels[j] != -1 {
                continue;
            }
            labels[j] = cluster_id;

            if neighbors[j].len() < min_pts {
                continue;
            }
            for &k in &neighbors[j] {
                if labels[k] == -1 {
                    queue.push_back(k);
                }
            }
        }
        cluster_id += 1;
    }
    labels
}

/// Assign each embedding to the most cosine-similar centroid, or `-1` if no
/// centroid is within `eps` cosine distance. Fast path for re-running over an
/// existing set of named people.
pub fn assign_to_centroids(embeddings: &[Vec<f32>], centroids: &[Vec<f32>], eps: f32) -> Vec<i32> {
    embeddings
        .iter()
        .map(|emb| {
            let mut best_sim = 0f32;
            let mut best_label = -1i32;
            for (ci, centroid) in centroids.iter().enumerate() {
                let sim = cosine_sim(emb, centroid);
                if sim > best_sim {
                    best_sim = sim;
                    best_label = ci as i32;
                }
            }
            if best_sim >= 1.0 - eps {
                best_label
            } else {
                -1
            }
        })
        .collect()
}

/// Mean of embeddings, L2-normalised. Empty input yields an empty vector.
pub fn compute_centroid(embeddings: &[Vec<f32>]) -> Vec<f32> {
    if embeddings.is_empty() {
        return Vec::new();
    }
    let dim = embeddings[0].len();
    let mut centroid = vec![0f32; dim];
    for emb in embeddings {
        for j in 0..dim {
            centroid[j] += emb[j];
        }
    }

    let n = embeddings.len() as f32;
    let mut norm = 0f32;
    for j in 0..dim {
        centroid[j] /= n;
        norm += centroid[j] * centroid[j];
    }
    norm = norm.sqrt();
    if norm > 0.0 {
        for c in &mut centroid {
            *c /= norm;
        }
    }
    centroid
}

fn precompute_neighbors(embeddings: &[Vec<f32>], eps: f32) -> Vec<Vec<usize>> {
    let n = embeddings.len();
    let threshold = 1.0 - eps;
    let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];

    for i in 0..n {
        for j in (i + 1)..n {
            if cosine_sim(&embeddings[i], &embeddings[j]) >= threshold {
                neighbors[i].push(j);
                neighbors[j].push(i);
            }
        }
    }
    neighbors
}

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0f32;
    let mut na = 0f32;
    let mut nb = 0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    na = na.sqrt();
    nb = nb.sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalize(v: &[f32]) -> Vec<f32> {
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / norm).collect()
    }

    mod dbscan {
        use super::*;

        #[test]
        fn two_clusters_separated_correctly() {
            let embeddings = vec![
                normalize(&[1.0, 0.0, 0.0]),
                normalize(&[0.95, 0.05, 0.0]),
                normalize(&[0.0, 0.0, 1.0]),
                normalize(&[0.05, 0.0, 0.95]),
            ];
            let labels = dbscan(&embeddings, 0.3, 1);

            assert_eq!(labels[0], labels[1]);
            assert_eq!(labels[2], labels[3]);
            assert_ne!(labels[0], labels[2]);
        }

        #[test]
        fn single_point_is_noise() {
            let embeddings = vec![
                normalize(&[1.0, 0.0, 0.0]),
                normalize(&[0.0, 1.0, 0.0]),
                normalize(&[0.0, 0.0, 1.0]),
            ];
            let labels = dbscan(&embeddings, 0.3, 2);

            assert!(labels.iter().all(|&l| l == -1));
        }

        #[test]
        fn all_similar_one_cluster() {
            let embeddings = vec![
                normalize(&[1.0, 0.0, 0.0]),
                normalize(&[0.99, 0.01, 0.0]),
                normalize(&[0.98, 0.02, 0.0]),
            ];
            let labels = dbscan(&embeddings, 0.3, 1);

            assert!(labels.iter().all(|&l| l == labels[0] && l >= 0));
        }

        #[test]
        fn empty_input_returns_empty() {
            let labels = dbscan(&[], 0.3, 2);
            assert!(labels.is_empty());
        }
    }

    mod assign_to_centroids {
        use super::*;

        #[test]
        fn matches_nearest() {
            let centroids = vec![normalize(&[1.0, 0.0, 0.0]), normalize(&[0.0, 0.0, 1.0])];
            let embeddings = vec![normalize(&[0.9, 0.1, 0.0]), normalize(&[0.1, 0.0, 0.9])];
            let labels = assign_to_centroids(&embeddings, &centroids, 0.4);

            assert_eq!(labels[0], 0);
            assert_eq!(labels[1], 1);
        }

        #[test]
        fn far_from_all_is_noise() {
            let centroids = vec![normalize(&[1.0, 0.0, 0.0])];
            let embeddings = vec![normalize(&[0.0, 1.0, 0.0])];
            let labels = assign_to_centroids(&embeddings, &centroids, 0.3);

            assert_eq!(labels[0], -1);
        }
    }

    mod compute_centroid {
        use super::*;

        #[test]
        fn is_normalized() {
            let embeddings = vec![normalize(&[1.0, 0.0, 0.0]), normalize(&[0.0, 1.0, 0.0])];
            let centroid = compute_centroid(&embeddings);

            let norm = centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((0.99..=1.01).contains(&norm));
        }
    }

    mod assemble_clusters {
        use crate::models::FaceEmbeddingRow;

        fn row(file_id: &str, x: f64, y: f64, vec: Vec<f32>) -> FaceEmbeddingRow {
            FaceEmbeddingRow {
                file_id: file_id.to_string(),
                bbox_x: x,
                bbox_y: y,
                bbox_w: 0.1,
                bbox_h: 0.1,
                vec,
            }
        }

        #[test]
        fn groups_clusters_and_routes_noise() {
            let rows = vec![
                row("a", 0.1, 0.1, vec![1.0, 0.0]),
                row("b", 0.2, 0.2, vec![1.0, 0.0]),
                row("c", 0.3, 0.3, vec![0.0, 1.0]),
                row("d", 0.4, 0.4, vec![0.5, 0.5]),
            ];
            let labels = [0, 0, 1, -1];
            let out = super::super::assemble_clusters(&rows, &labels);

            // 3 clustered members + 1 noise member
            assert_eq!(out.members.len(), 4);
            // 2 real clusters → 2 centroids (noise has none)
            assert_eq!(out.centroids.len(), 2);

            let noise: Vec<_> = out
                .members
                .iter()
                .filter(|m| m.cluster_id == super::super::NOISE_CLUSTER_ID)
                .collect();
            assert_eq!(noise.len(), 1);
            assert_eq!(noise[0].file_id, "d");
        }

        #[test]
        fn cluster_id_is_stable_and_order_independent() {
            let rows_ab = vec![row("a", 0.1, 0.1, vec![1.0]), row("b", 0.2, 0.2, vec![1.0])];
            let rows_ba = vec![row("b", 0.2, 0.2, vec![1.0]), row("a", 0.1, 0.1, vec![1.0])];

            let id1 = super::super::assemble_clusters(&rows_ab, &[0, 0]).members[0].cluster_id.clone();
            let id2 = super::super::assemble_clusters(&rows_ba, &[0, 0]).members[0].cluster_id.clone();

            assert_eq!(id1, id2);
            assert!(id1.starts_with("face-"));
            assert_eq!(id1.len(), "face-".len() + 16);
        }
    }
}
