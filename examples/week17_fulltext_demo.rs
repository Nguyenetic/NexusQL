// Week 17: Full-Text Search Demo
// Demonstrates tsvector, tsquery, and full-text search capabilities

use nexusql::fulltext::{FullTextSearch, TSQuery};

fn main() {
    println!("🚀 NexusQL Week 17: Full-Text Search Demo");
    println!("=========================================\n");

    demo_basic_search();
    demo_boolean_operators();
    demo_phrase_search();
    demo_ranking();
    demo_stop_words();

    println!("\n✅ All demos completed successfully!");
}

fn demo_basic_search() {
    println!("🔍 Demo 1: Basic Full-Text Search");
    println!("----------------------------------\n");

    let documents = vec![
        "PostgreSQL is a powerful open source database",
        "Rust programming language ensures memory safety",
        "Database indexing improves query performance",
        "The quick brown fox jumps over the lazy dog",
    ];

    println!("Documents:");
    for (i, doc) in documents.iter().enumerate() {
        println!("  {}: {}", i + 1, doc);
    }

    // Search for "database"
    println!("\n🔎 Search: \"database\"");
    let query = FullTextSearch::to_tsquery("database").unwrap();

    for (i, doc) in documents.iter().enumerate() {
        let vec = FullTextSearch::to_tsvector(doc);
        if vec.matches(&query) {
            println!("  ✓ Match found in document {}", i + 1);
        }
    }

    // Search for "programming"
    println!("\n🔎 Search: \"programming\"");
    let query = FullTextSearch::to_tsquery("programming").unwrap();

    for (i, doc) in documents.iter().enumerate() {
        let vec = FullTextSearch::to_tsvector(doc);
        if vec.matches(&query) {
            println!("  ✓ Match found in document {}", i + 1);
        }
    }
}

fn demo_boolean_operators() {
    println!("\n\n🔗 Demo 2: Boolean Operators (AND, OR, NOT)");
    println!("--------------------------------------------\n");

    let doc = "PostgreSQL is a powerful database system with advanced features";
    let vec = FullTextSearch::to_tsvector(doc);

    println!("Document: {}\n", doc);

    // AND operator
    let q_and = FullTextSearch::plainto_tsquery("database system").unwrap();
    println!("Query: database AND system");
    println!("  Matches: {}\n", vec.matches(&q_and));

    // OR operator
    let q1 = FullTextSearch::to_tsquery("database").unwrap();
    let q2 = FullTextSearch::to_tsquery("python").unwrap();
    let q_or = q1.or(q2);
    println!("Query: database OR python");
    println!("  Matches: {}\n", vec.matches(&q_or));

    // NOT operator
    let q_cat = FullTextSearch::to_tsquery("cat").unwrap();
    let q_not = q_cat.not();
    println!("Query: NOT cat");
    println!("  Matches: {} (cat not in document)", vec.matches(&q_not));
}

fn demo_phrase_search() {
    println!("\n\n📝 Demo 3: Phrase Search (FOLLOWED BY)");
    println!("---------------------------------------\n");

    let documents = vec!["the quick brown fox", "brown quick fox", "fox brown quick"];

    println!("Documents:");
    for (i, doc) in documents.iter().enumerate() {
        println!("  {}: {}", i + 1, doc);
    }

    // Phrase: "quick brown"
    println!("\n🔎 Phrase: \"quick brown\" (order matters)");
    let query = FullTextSearch::phraseto_tsquery("quick brown").unwrap();

    for (i, doc) in documents.iter().enumerate() {
        let vec = FullTextSearch::to_tsvector(doc);
        if vec.matches(&query) {
            println!("  ✓ Match in document {}: consecutive words", i + 1);
        } else {
            println!("  ✗ No match in document {}: wrong order", i + 1);
        }
    }
}

fn demo_ranking() {
    println!("\n\n⭐ Demo 4: Relevance Ranking (ts_rank)");
    println!("---------------------------------------\n");

    let documents = vec![
        ("Doc 1", "database database database"),
        ("Doc 2", "database systems and database technology"),
        ("Doc 3", "introduction to databases"),
    ];

    let query = FullTextSearch::to_tsquery("database").unwrap();

    println!("Search query: \"database\"\n");
    println!("Ranked results:");

    let mut ranked: Vec<_> = documents
        .iter()
        .map(|(name, doc)| {
            let vec = FullTextSearch::to_tsvector(doc);
            let rank = FullTextSearch::ts_rank(&vec, &query);
            (*name, rank)
        })
        .collect();

    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    for (name, rank) in ranked {
        println!("  {} - Rank: {:.4}", name, rank);
    }

    println!("\n  Higher rank = more relevant (based on term frequency)");
}

fn demo_stop_words() {
    println!("\n\n🚫 Demo 5: Stop Words Filtering");
    println!("--------------------------------\n");

    let text = "the quick brown fox is a fast animal and it jumps over the lazy dog";
    let vec = FullTextSearch::to_tsvector(text);

    println!("Original text:");
    println!("  {}\n", text);

    println!("After stop word removal:");
    let lexemes: Vec<_> = vec.lexemes.iter().map(|l| &l.word).collect();
    println!("  Lexemes: {:?}\n", lexemes);

    println!("Stop words removed:");
    println!("  the, is, a, and, it, over");
    println!("\nContent words preserved:");
    println!("  quick, brown, fox, fast, animal, jump, lazy, dog");

    println!(
        "\n  Total lexemes: {} (reduced from ~16 words)",
        vec.length()
    );
}
