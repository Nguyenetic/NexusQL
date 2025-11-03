// NexusQL v2.0.0 - AI-Native Database Demo
// Vector embeddings, hybrid search, AutoML optimization

use nexusql::ai_native::{
    AutoMLOptimizer, Embedding, EmbeddingGenerator, HybridSearch, QueryExecution,
};

fn main() {
    println!("🤖 NexusQL v2.0.0 - AI-Native Database Demo");
    println!("==========================================\n");

    demo_embeddings();
    demo_semantic_search();
    demo_hybrid_search();
    demo_automl_optimization();

    println!("\n🎉 v2.0.0 - AI-Native Database Complete!");
    println!("\n✨ NexusQL Evolution Complete:");
    println!("   v1.0.0 → Production Database");
    println!("   v1.1.0 → Extensible Platform");
    println!("   v1.2.0 → Analytics Powerhouse");
    println!("   v1.3.0 → Planet-Scale Distribution");
    println!("   v2.0.0 → AI-Native Intelligence 🤖");
}

fn demo_embeddings() {
    println!("🧠 Demo 1: Vector Embeddings");
    println!("----------------------------\n");

    // Generate embeddings from text
    println!("Generating embeddings from text...");
    let emb1 = EmbeddingGenerator::from_text("machine learning", 128);
    let emb2 = EmbeddingGenerator::from_text("artificial intelligence", 128);
    let emb3 = EmbeddingGenerator::from_text("database systems", 128);

    println!("  ✓ 'machine learning' → 128-dim vector");
    println!("  ✓ 'artificial intelligence' → 128-dim vector");
    println!("  ✓ 'database systems' → 128-dim vector\n");

    // Calculate similarities
    println!("Semantic similarities (cosine):");
    let sim_ml_ai = emb1.cosine_similarity(&emb2);
    let sim_ml_db = emb1.cosine_similarity(&emb3);

    println!(
        "  'machine learning' ↔ 'artificial intelligence': {:.3}",
        sim_ml_ai
    );
    println!(
        "  'machine learning' ↔ 'database systems': {:.3}",
        sim_ml_db
    );

    println!("\n  💡 Similar concepts have higher similarity scores!");
    println!("  💡 Automatic semantic understanding of text!");
}

fn demo_semantic_search() {
    println!("\n\n🔍 Demo 2: Semantic Search (Vector Similarity)");
    println!("-----------------------------------------------\n");

    // Create document embeddings
    let documents = vec![
        ("PostgreSQL is a powerful relational database", "doc1"),
        ("Machine learning models require training data", "doc2"),
        ("Python is great for data science", "doc3"),
        ("SQL queries retrieve data from databases", "doc4"),
        ("Neural networks learn from examples", "doc5"),
    ];

    println!("Document collection ({} docs):", documents.len());
    for (doc, id) in &documents {
        println!("  {}: {}", id, doc);
    }

    // Generate embeddings
    let doc_embeddings: Vec<_> = documents
        .iter()
        .map(|(text, id)| (EmbeddingGenerator::from_text(text, 64), *id))
        .collect();

    // Search query
    let query = "database query language";
    let query_emb = EmbeddingGenerator::from_text(query, 64);

    println!("\n🔎 Query: '{}'\n", query);
    println!("Semantic search results (by similarity):");

    let mut results: Vec<_> = doc_embeddings
        .iter()
        .map(|(emb, id)| {
            let score = query_emb.cosine_similarity(emb);
            (id, score)
        })
        .collect();

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    for (i, (id, score)) in results.iter().take(3).enumerate() {
        println!("  {}. {} (score: {:.3})", i + 1, id, score);
    }

    println!("\n  💡 Finds semantically similar documents!");
    println!("  💡 Works even with different wording!");
}

fn demo_hybrid_search() {
    println!("\n\n🎯 Demo 3: Hybrid Search (Semantic + Keyword)");
    println!("----------------------------------------------\n");

    let documents = vec![
        (
            EmbeddingGenerator::from_text("PostgreSQL database tutorial", 64),
            "PostgreSQL database tutorial for beginners".to_string(),
        ),
        (
            EmbeddingGenerator::from_text("Python programming guide", 64),
            "Python programming guide and examples".to_string(),
        ),
        (
            EmbeddingGenerator::from_text("Machine learning with Python", 64),
            "Machine learning with Python and databases".to_string(),
        ),
    ];

    let query_text = "database Python";
    let query_emb = EmbeddingGenerator::from_text(query_text, 64);

    println!("Query: '{}'\n", query_text);

    // Pure semantic search (alpha = 1.0)
    println!("Pure Semantic Search (alpha=1.0):");
    let semantic = HybridSearch::new(1.0);
    let results_sem = semantic.search(&query_emb, query_text, &documents);
    for (i, (idx, score)) in results_sem.iter().take(2).enumerate() {
        println!("  {}. Doc {} (score: {:.3})", i + 1, idx, score);
    }

    // Pure keyword search (alpha = 0.0)
    println!("\nPure Keyword Search (alpha=0.0):");
    let keyword = HybridSearch::new(0.0);
    let results_key = keyword.search(&query_emb, query_text, &documents);
    for (i, (idx, score)) in results_key.iter().take(2).enumerate() {
        println!("  {}. Doc {} (score: {:.3})", i + 1, idx, score);
    }

    // Hybrid search (alpha = 0.5)
    println!("\nHybrid Search (alpha=0.5, 50/50):");
    let hybrid = HybridSearch::new(0.5);
    let results_hyb = hybrid.search(&query_emb, query_text, &documents);
    for (i, (idx, score)) in results_hyb.iter().take(2).enumerate() {
        println!("  {}. Doc {} (score: {:.3})", i + 1, idx, score);
    }

    println!("\n  💡 Hybrid = best of both semantic + keyword!");
    println!("  💡 Tune alpha (0-1) for your use case!");
}

fn demo_automl_optimization() {
    println!("\n\n🎓 Demo 4: AutoML Query Optimization");
    println!("-------------------------------------\n");

    let mut optimizer = AutoMLOptimizer::new();

    println!("Learning from query executions...\n");

    // Simulate query executions
    let queries = vec![
        ("SELECT * FROM users WHERE id = 1", 15.0, None),
        ("SELECT * FROM users WHERE id = 2", 14.0, None),
        (
            "SELECT * FROM users WHERE id = 3",
            2.0,
            Some("users_id_idx"),
        ),
        (
            "SELECT * FROM products WHERE category = 'books'",
            50.0,
            None,
        ),
        (
            "SELECT * FROM products WHERE category = 'electronics'",
            3.0,
            Some("products_cat_idx"),
        ),
    ];

    for (query, time, index) in queries {
        optimizer.record_execution(QueryExecution {
            query: query.to_string(),
            execution_time_ms: time,
            rows_scanned: 1000,
            rows_returned: 10,
            index_used: index.map(String::from),
        });

        if let Some(idx) = index {
            println!("  ✓ Query with {} : {:.1} ms", idx, time);
        } else {
            println!("  ⚠ Query without index: {:.1} ms", time);
        }
    }

    let stats = optimizer.learning_stats();
    println!("\nAutoML Learning Stats:");
    println!("  Queries learned: {}", stats.queries_learned);
    println!("  Patterns identified: {}", stats.patterns_identified);
    println!(
        "  Optimizations suggested: {}",
        stats.optimizations_suggested
    );

    println!("\n  💡 Learns optimal indexes from execution history!");
    println!("  💡 Automatically suggests performance improvements!");
    println!("  💡 Adaptive query optimization over time!");
}
