//! Week 11: Advanced Indexing Demo
//!
//! Demonstrates the three new PostgreSQL-compatible index types:
//! 1. GIN (Generalized Inverted Index) - for arrays, JSON, full-text search
//! 2. GiST (Generalized Search Tree) - for geometric data and ranges
//! 3. BRIN (Block Range Index) - for very large sequential tables
//!
//! Run with: cargo run --example week11_advanced_indexes_demo

use nexusql::index::{
    BRINIndex, GINIndex, GINKey, GINOperatorClass, GiSTIndex, GiSTIndexType, GiSTPredicate,
};
use nexusql::types::Value;

fn main() {
    println!("🚀 NexusQL Week 11: Advanced Indexing Demo");
    println!("============================================\n");

    demo_gin_array_indexing();
    demo_gin_full_text_search();
    demo_gist_spatial_indexing();
    demo_gist_range_indexing();
    demo_brin_time_series();
    demo_index_comparison();

    println!("\n✅ All demos completed successfully!");
}

/// Demo 1: GIN Index for Array Operations
fn demo_gin_array_indexing() {
    println!("📊 Demo 1: GIN Index - Array Containment & Overlap");
    println!("---------------------------------------------------");

    let mut index = GINIndex::new(
        "product_tags_idx".to_string(),
        "tags".to_string(),
        GINOperatorClass::ArrayOps,
    );

    // Insert product tags (comma-separated arrays)
    let products = vec![
        (0, "laptop,electronics,computer,portable"),
        (1, "smartphone,electronics,mobile,android"),
        (2, "desk,furniture,office,wood"),
        (3, "chair,furniture,office,ergonomic"),
        (4, "monitor,electronics,display,4k"),
    ];

    println!("\n📝 Indexing {} products with tags...", products.len());
    for (id, tags) in &products {
        index.insert(*id, &Value::Text(tags.to_string()));
        println!("  Product {}: {}", id, tags);
    }

    // Search for products containing "electronics"
    println!("\n🔍 Search: Products with 'electronics' tag");
    let keys = vec![GINKey::Text("electronics".to_string())];
    let results = index.search_contains_all(&keys);
    println!("  Found {} products: {:?}", results.len(), results);

    // Search for products with both "electronics" AND "portable"
    println!("\n🔍 Search: Products with 'electronics' AND 'portable'");
    let keys = vec![
        GINKey::Text("electronics".to_string()),
        GINKey::Text("portable".to_string()),
    ];
    let results = index.search_contains_all(&keys);
    println!("  Found {} products: {:?}", results.len(), results);

    // Search for products overlapping with ["furniture", "office"]
    println!("\n🔍 Search: Products overlapping with ['furniture', 'office']");
    let keys = vec![
        GINKey::Text("furniture".to_string()),
        GINKey::Text("office".to_string()),
    ];
    let results = index.search_overlaps(&keys);
    println!("  Found {} products: {:?}", results.len(), results);

    // Show index statistics
    let stats = index.stats();
    println!("\n📈 GIN Index Statistics:");
    println!("  Total keys indexed: {}", stats.total_keys);
    println!("  Total postings: {}", stats.total_postings);
    println!(
        "  Avg posting list size: {:.2}",
        stats.avg_posting_list_size
    );
    println!("  Entries indexed: {}", stats.entries_indexed);
}

/// Demo 2: GIN Index for Full-Text Search
fn demo_gin_full_text_search() {
    println!("\n\n📚 Demo 2: GIN Index - Full-Text Search");
    println!("----------------------------------------");

    let mut index = GINIndex::new(
        "article_content_idx".to_string(),
        "content".to_string(),
        GINOperatorClass::TsvectorOps,
    );

    // Insert article content
    let articles = vec![
        (0, "The quick brown fox jumps over the lazy dog"),
        (1, "PostgreSQL is a powerful open source database"),
        (2, "Rust programming language ensures memory safety"),
        (
            3,
            "Database indexing improves query performance significantly",
        ),
        (4, "The lazy cat sleeps on the warm sunny windowsill"),
    ];

    println!("\n📝 Indexing {} articles...", articles.len());
    for (id, content) in &articles {
        index.insert(*id, &Value::Text(content.to_string()));
        println!("  Article {}: \"{}\"", id, content);
    }

    // Search for "database"
    println!("\n🔍 Search: Articles containing 'database'");
    let keys = vec![GINKey::Text("database".to_string())];
    let results = index.search_contains_all(&keys);
    println!("  Found {} articles: {:?}", results.len(), results);

    // Search for "lazy" (should find articles 0 and 4)
    println!("\n🔍 Search: Articles containing 'lazy'");
    let keys = vec![GINKey::Text("lazy".to_string())];
    let results = index.search_contains_all(&keys);
    println!("  Found {} articles: {:?}", results.len(), results);

    // Search for "database" AND "performance"
    println!("\n🔍 Search: Articles with 'database' AND 'performance'");
    let keys = vec![
        GINKey::Text("database".to_string()),
        GINKey::Text("performance".to_string()),
    ];
    let results = index.search_contains_all(&keys);
    println!("  Found {} articles: {:?}", results.len(), results);

    let stats = index.stats();
    println!("\n📈 Full-Text Index Statistics:");
    println!("  Unique tokens: {}", stats.total_keys);
    println!("  Total token occurrences: {}", stats.total_postings);
}

/// Demo 3: GiST Index for Spatial/Geometric Data
fn demo_gist_spatial_indexing() {
    println!("\n\n🗺️  Demo 3: GiST Index - Spatial/Geometric Queries");
    println!("--------------------------------------------------");

    let mut index = GiSTIndex::new(
        "store_location_idx".to_string(),
        "coordinates".to_string(),
        GiSTIndexType::RTree,
    );

    // Insert store locations (x, y coordinates)
    let stores = vec![
        (0, "10.5,20.3"), // Downtown store
        (1, "15.2,25.8"), // North store
        (2, "8.1,18.5"),  // West store
        (3, "30.5,40.2"), // Far east store
        (4, "12.0,22.5"), // Central store
    ];

    println!("\n📝 Indexing {} store locations...", stores.len());
    for (id, coords) in &stores {
        index.insert(*id, &Value::Text(coords.to_string()));
        println!(
            "  Store {}: ({}, {})",
            id,
            coords.split(',').nth(0).unwrap(),
            coords.split(',').nth(1).unwrap()
        );
    }

    // Search for stores within bounding box [0,0] to [20,30]
    println!("\n🔍 Search: Stores within bounding box [0,0] to [20,30]");
    let query = GiSTPredicate::BoundingBox {
        min_x: 0.0,
        max_x: 20.0,
        min_y: 0.0,
        max_y: 30.0,
    };
    let results = index.search(&query);
    println!("  Found {} stores: {:?}", results.len(), results);

    // Search for stores within smaller area [10,20] to [16,26]
    println!("\n🔍 Search: Stores within bounding box [10,20] to [16,26]");
    let query = GiSTPredicate::BoundingBox {
        min_x: 10.0,
        max_x: 16.0,
        min_y: 20.0,
        max_y: 26.0,
    };
    let results = index.search(&query);
    println!("  Found {} stores: {:?}", results.len(), results);

    let stats = index.stats();
    println!("\n📈 GiST Spatial Index Statistics:");
    println!("  Entries indexed: {}", stats.entries_indexed);
    println!("  Tree height: {}", stats.tree_height);
}

/// Demo 4: GiST Index for Range Queries
fn demo_gist_range_indexing() {
    println!("\n\n📊 Demo 4: GiST Index - Range Queries");
    println!("-------------------------------------");

    let mut index = GiSTIndex::new(
        "employee_age_idx".to_string(),
        "age".to_string(),
        GiSTIndexType::RangeTree,
    );

    // Insert employee ages
    let employees = vec![
        (0, 25),
        (1, 30),
        (2, 35),
        (3, 40),
        (4, 45),
        (5, 50),
        (6, 28),
        (7, 33),
        (8, 38),
        (9, 42),
    ];

    println!("\n📝 Indexing {} employees by age...", employees.len());
    for (id, age) in &employees {
        index.insert(*id, &Value::Integer(*age));
        println!("  Employee {}: {} years old", id, age);
    }

    // Search for employees aged 30-40
    println!("\n🔍 Search: Employees aged 30-40");
    let query = GiSTPredicate::IntRange { min: 30, max: 40 };
    let results = index.search(&query);
    println!("  Found {} employees: {:?}", results.len(), results);

    // Search for employees aged 35-45
    println!("\n🔍 Search: Employees aged 35-45");
    let query = GiSTPredicate::IntRange { min: 35, max: 45 };
    let results = index.search(&query);
    println!("  Found {} employees: {:?}", results.len(), results);

    let stats = index.stats();
    println!("\n📈 GiST Range Index Statistics:");
    println!("  Entries indexed: {}", stats.entries_indexed);
    println!("  Tree height: {}", stats.tree_height);
}

/// Demo 5: BRIN Index for Time-Series Data
fn demo_brin_time_series() {
    println!("\n\n📈 Demo 5: BRIN Index - Time-Series/Sequential Data");
    println!("---------------------------------------------------");

    let mut index = BRINIndex::new(
        "events_timestamp_idx".to_string(),
        "timestamp".to_string(),
        Some(10), // 10 pages per range (small for demo)
    );

    // Insert sequential timestamp data (simulating time-series)
    println!("\n📝 Inserting 1,000 sequential events...");
    for i in 0..1000 {
        index.insert(i, &Value::Integer(i as i64));
    }
    println!("  ✓ Indexed 1,000 events");

    // Search for events in range [250, 350]
    println!("\n🔍 Search: Events with timestamp 250-350");
    let results = index.search_range(&Value::Integer(250), &Value::Integer(350));
    println!("  Found {} candidate events", results.len());
    println!("  Note: BRIN returns block ranges (may include false positives)");

    // Search for events > 800
    println!("\n🔍 Search: Events with timestamp > 800");
    let results = index.search_greater_than(&Value::Integer(800));
    println!("  Found {} candidate events", results.len());

    let stats = index.stats();
    println!("\n📈 BRIN Index Statistics:");
    println!("  Total ranges: {}", stats.total_ranges);
    println!("  Pages per range: {}", stats.pages_per_range);
    println!("  Entries indexed: {}", stats.entries_indexed);
    println!("  Index size: {} bytes", stats.index_size_bytes);
    println!(
        "  Compression ratio vs B-Tree: {:.1}x",
        stats.compression_ratio
    );
    println!(
        "\n  💡 BRIN is {:.0}x smaller than B-Tree!",
        stats.compression_ratio
    );
}

/// Demo 6: Index Type Comparison
fn demo_index_comparison() {
    println!("\n\n🔬 Demo 6: Index Type Comparison");
    println!("=================================");

    println!("\n┌─────────────────────────────────────────────────────────────┐");
    println!("│ Index Type │ Best For                    │ Space Efficiency  │");
    println!("├─────────────────────────────────────────────────────────────┤");
    println!("│ B-Tree     │ Ordered data, ranges         │ Standard (1x)     │");
    println!("│ HNSW       │ Vector similarity search     │ Large (10-20x)    │");
    println!("│ GIN        │ Arrays, JSON, full-text      │ Medium (2-5x)     │");
    println!("│ GiST       │ Geometric, spatial, ranges   │ Medium (3-6x)     │");
    println!("│ BRIN       │ Sequential, time-series      │ Tiny (100-1000x)  │");
    println!("└─────────────────────────────────────────────────────────────┘");

    println!("\n📊 Use Case Examples:");
    println!("  • E-commerce tags          → GIN (array containment)");
    println!("  • Document search          → GIN (full-text search)");
    println!("  • Store locations          → GiST (spatial queries)");
    println!("  • Age ranges               → GiST (range queries)");
    println!("  • Log timestamps           → BRIN (sequential data)");
    println!("  • IoT sensor data          → BRIN (time-series)");
    println!("  • Product catalog          → B-Tree (traditional)");
    println!("  • Image similarity         → HNSW (vector search)");

    println!("\n💡 Key Insights:");
    println!("  1. GIN excels at multi-valued attributes (arrays, tokens)");
    println!("  2. GiST handles geometric and custom predicates");
    println!("  3. BRIN offers massive space savings for sorted data");
    println!("  4. Choose based on query patterns, not just data type");
}
