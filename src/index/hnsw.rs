//! HNSW Index - Fixed Implementation
//! PROPRIETARY

use std::collections::{BinaryHeap, HashSet};
use std::cmp::Ordering;
use ndarray::Array1;

#[derive(Clone)]
struct Neighbor {
    id: usize,
    distance: f32,
}

impl PartialEq for Neighbor {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for Neighbor {}

impl PartialOrd for Neighbor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.distance.partial_cmp(&self.distance)
    }
}

impl Ord for Neighbor {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HNSWIndex {
    vectors: Vec<Array1<f32>>,
    graph: Vec<Vec<usize>>,  // Simplified: single-layer for now
    entry_point: usize,
    m: usize,
    ef_construction: usize,
    ef_search: usize,
}

impl HNSWIndex {
    pub fn new(m: usize, ef_construction: usize, ef_search: usize) -> Self {
        Self {
            vectors: Vec::new(),
            graph: Vec::new(),
            entry_point: 0,
            m,
            ef_construction,
            ef_search,
        }
    }
    
    pub fn insert(&mut self, vector: Array1<f32>) -> usize {
        let new_id = self.vectors.len();
        self.vectors.push(vector);
        self.graph.push(Vec::new());
        
        if new_id == 0 {
            self.entry_point = 0;
            return 0;
        }
        
        // Connect to nearest existing neighbors
        let nearest = self.find_nearest(new_id, self.m);
        
        for neighbor_id in &nearest {
            // Bidirectional connection
            self.graph[new_id].push(*neighbor_id);
            self.graph[*neighbor_id].push(new_id);
            
            // Prune if too many connections
            if self.graph[*neighbor_id].len() > self.m * 2 {
                self.prune_connections(*neighbor_id);
            }
        }
        
        new_id
    }
    
    fn find_nearest(&self, query_id: usize, k: usize) -> Vec<usize> {
        let query = &self.vectors[query_id];
        let mut distances: Vec<(usize, f32)> = (0..query_id)
            .map(|id| {
                let dist = euclidean_distance(&self.vectors[id], query);
                (id, dist)
            })
            .collect();
        
        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        distances.truncate(k);
        
        distances.into_iter().map(|(id, _)| id).collect()
    }
    
    fn prune_connections(&mut self, node_id: usize) {
        let neighbors = &mut self.graph[node_id];
        if neighbors.len() <= self.m {
            return;
        }
        
        // Keep only M closest neighbors
        let node_vec = &self.vectors[node_id];
        neighbors.sort_by(|a, b| {
            let dist_a = euclidean_distance(&self.vectors[*a], node_vec);
            let dist_b = euclidean_distance(&self.vectors[*b], node_vec);
            dist_a.partial_cmp(&dist_b).unwrap()
        });
        neighbors.truncate(self.m);
    }
    
    pub fn search(&self, query: &Array1<f32>, k: usize) -> Vec<(usize, f32)> {
        if self.vectors.is_empty() {
            return vec![];
        }
        
        let mut visited = HashSet::new();
        let mut candidates = BinaryHeap::new();
        
        let entry_dist = euclidean_distance(&self.vectors[self.entry_point], query);
        candidates.push(Neighbor {
            id: self.entry_point,
            distance: entry_dist,
        });
        visited.insert(self.entry_point);
        
        let mut results = Vec::new();
        
        while let Some(current) = candidates.pop() {
            results.push((current.id, current.distance));
            
            for &neighbor_id in &self.graph[current.id] {
                if visited.insert(neighbor_id) {
                    let dist = euclidean_distance(&self.vectors[neighbor_id], query);
                    candidates.push(Neighbor {
                        id: neighbor_id,
                        distance: dist,
                    });
                }
            }
            
            if results.len() >= self.ef_search.max(k) {
                break;
            }
        }
        
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        results.truncate(k);
        results
    }
    
    pub fn len(&self) -> usize {
        self.vectors.len()
    }
    
    pub fn set_ef(&mut self, ef: usize) {
        self.ef_search = ef;
    }
}

fn euclidean_distance(a: &Array1<f32>, b: &Array1<f32>) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::arr1;
    
    #[test]
    fn test_hnsw_basic() {
        let mut index = HNSWIndex::new(16, 200, 50);
        
        index.insert(arr1(&[1.0, 0.0, 0.0]));
        index.insert(arr1(&[0.0, 1.0, 0.0]));
        index.insert(arr1(&[0.0, 0.0, 1.0]));
        
        assert_eq!(index.len(), 3);
        
        let query = arr1(&[0.9, 0.1, 0.0]);
        let results = index.search(&query, 2);
        
        assert!(results.len() >= 1);
        assert!(results.len() <= 2);
    }
    
    #[test]
    fn test_hnsw_recall() {
        let mut index = HNSWIndex::new(16, 200, 50);
        
        let mut vectors = Vec::new();
        for i in 0..100 {
            let v = arr1(&[i as f32, (i * 2) as f32, (i * 3) as f32]);
            vectors.push(v.clone());
            index.insert(v);
        }
        
        let mut correct = 0;
        for (i, vec) in vectors.iter().enumerate() {
            let results = index.search(vec, 1);
            if !results.is_empty() && results[0].0 == i {
                correct += 1;
            }
        }
        
        let recall = correct as f32 / vectors.len() as f32;
        println!("HNSW Recall@1: {:.1}%", recall * 100.0);
        
        assert!(recall > 0.8, "Recall too low: {}", recall);
    }
    
    #[test]
    fn test_ef_parameter() {
        let mut index = HNSWIndex::new(16, 200, 10);
        
        for i in 0..50 {
            index.insert(arr1(&[i as f32, 0.0, 0.0]));
        }
        
        let query = arr1(&[25.0, 0.0, 0.0]);
        
        index.set_ef(10);
        let results_low = index.search(&query, 5);
        
        index.set_ef(50);
        let results_high = index.search(&query, 5);
        
        assert!(results_low.len() > 0);
        assert!(results_high.len() > 0);
    }
}
