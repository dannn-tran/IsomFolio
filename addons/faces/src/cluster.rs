use std::collections::VecDeque;

pub fn dbscan(embeddings: &[Vec<f32>], eps: f32, min_pts: usize) -> Vec<i32> {
    let n = embeddings.len();
    let mut labels = vec![-1i32; n];
    let mut visited = vec![false; n];
    let mut cluster_id = 0i32;

    for i in 0..n {
        if visited[i] {
            continue;
        }
        visited[i] = true;

        let neighbors = region_query(embeddings, i, eps);
        if neighbors.len() < min_pts {
            continue;
        }

        labels[i] = cluster_id;
        let mut queue: VecDeque<usize> = neighbors.into_iter().filter(|&j| j != i).collect();

        while let Some(j) = queue.pop_front() {
            if labels[j] == -1 {
                labels[j] = cluster_id;
            }
            if visited[j] {
                continue;
            }
            visited[j] = true;
            let new_neighbors = region_query(embeddings, j, eps);
            if new_neighbors.len() >= min_pts {
                for nb in new_neighbors {
                    if !visited[nb] {
                        queue.push_back(nb);
                    }
                }
            }
        }

        cluster_id += 1;
    }

    labels
}

fn region_query(embeddings: &[Vec<f32>], i: usize, eps: f32) -> Vec<usize> {
    embeddings
        .iter()
        .enumerate()
        .filter(|(_, e)| cosine_distance(&embeddings[i], e) <= eps)
        .map(|(j, _)| j)
        .collect()
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    1.0 - a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(v: Vec<f32>) -> Vec<f32> {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.into_iter().map(|x| x / norm).collect()
    }

    #[test]
    fn two_tight_clusters() {
        let a1 = unit(vec![1.0, 0.0, 0.0]);
        let a2 = unit(vec![0.98, 0.02, 0.0]);
        let b1 = unit(vec![0.0, 1.0, 0.0]);
        let b2 = unit(vec![0.01, 0.99, 0.0]);
        let embeddings = vec![a1, a2, b1, b2];
        let labels = dbscan(&embeddings, 0.1, 2);
        assert_eq!(labels[0], labels[1]);
        assert_eq!(labels[2], labels[3]);
        assert_ne!(labels[0], labels[2]);
        assert!(labels[0] >= 0);
    }

    #[test]
    fn noise_points_unlabelled() {
        let a = unit(vec![1.0, 0.0]);
        let b = unit(vec![0.0, 1.0]);
        let c = unit(vec![-1.0, 0.0]);
        let embeddings = vec![a, b, c];
        let labels = dbscan(&embeddings, 0.05, 2);
        assert!(labels.iter().all(|&l| l == -1));
    }
}
