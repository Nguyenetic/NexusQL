// Performance Benchmarks
// Validates real-world performance claims

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nexusql::analytics::VectorizedExecutor;
use nexusql::fulltext::FullTextSearch;
use nexusql::index::btree::BTreeIndex;
use nexusql::join::{JoinAlgorithm, JoinCondition, JoinExecutor, JoinType};
use nexusql::partition::{PartitionBounds, PartitionStrategy, PartitionedTable};
use nexusql::types::{Row, Value};

fn benchmark_btree_insert(c: &mut Criterion) {
    c.bench_function("btree_insert_1000", |b| {
        b.iter(|| {
            let mut index = BTreeIndex::new("test_idx".to_string(), "id".to_string());
            for i in 0..1000 {
                let row = Row::new(vec!["id".to_string()], vec![Value::Integer(i)]);
                index.insert(&row, i as usize).unwrap();
            }
        });
    });
}

fn benchmark_hash_join(c: &mut Criterion) {
    let left: Vec<Row> = (0..1000)
        .map(|i| Row::new(vec!["id".to_string()], vec![Value::Integer(i)]))
        .collect();

    let right: Vec<Row> = (500..1500)
        .map(|i| Row::new(vec!["id".to_string()], vec![Value::Integer(i)]))
        .collect();

    c.bench_function("hash_join_1000x1000", |b| {
        b.iter(|| {
            let executor = JoinExecutor::new(
                JoinType::Inner,
                JoinAlgorithm::Hash,
                JoinCondition::On {
                    left_column: "id".to_string(),
                    right_column: "id".to_string(),
                },
            );
            executor.execute(black_box(left.clone()), black_box(right.clone()))
        });
    });
}

fn benchmark_partition_insert(c: &mut Criterion) {
    c.bench_function("partitioned_insert_1000", |b| {
        b.iter(|| {
            let table = PartitionedTable::new(
                "test".to_string(),
                PartitionStrategy::Hash,
                "id".to_string(),
            );

            for i in 0..4 {
                table
                    .add_partition(
                        format!("p{}", i),
                        PartitionBounds::Hash {
                            modulus: 4,
                            remainder: i,
                        },
                    )
                    .unwrap();
            }

            for i in 0..1000 {
                let row = Row::new(vec!["id".to_string()], vec![Value::Integer(i)]);
                table.insert(row).unwrap();
            }
        });
    });
}

fn benchmark_fulltext_search(c: &mut Criterion) {
    let documents: Vec<_> = (0..100)
        .map(|i| {
            format!(
                "Document {} contains various terms like database query optimization",
                i
            )
        })
        .collect();

    let vectors: Vec<_> = documents
        .iter()
        .map(|doc| FullTextSearch::to_tsvector(doc))
        .collect();

    c.bench_function("fulltext_search_100_docs", |b| {
        b.iter(|| {
            let query = FullTextSearch::plainto_tsquery("database optimization").unwrap();
            vectors.iter().filter(|vec| vec.matches(&query)).count()
        });
    });
}

fn benchmark_vectorized_sum(c: &mut Criterion) {
    let values: Vec<i64> = (1..=10000).collect();

    c.bench_function("vectorized_sum_10k", |b| {
        b.iter(|| VectorizedExecutor::sum_vectorized(black_box(&values)));
    });
}

criterion_group!(
    benches,
    benchmark_btree_insert,
    benchmark_hash_join,
    benchmark_partition_insert,
    benchmark_fulltext_search,
    benchmark_vectorized_sum
);
criterion_main!(benches);
