//! Face embedding clustering. Ported from the C# Faces extension's `Clustering`
//! class; the inference engine returns raw embeddings and the host clusters them.

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
}
