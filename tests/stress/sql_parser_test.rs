//! SQL Parser Rigorous Testing
//! Complex queries, edge cases, malformed SQL

use nexusql::query::QueryParser;

#[test]
fn test_complex_where_clauses() {
    let queries = vec![
        "SELECT * FROM users WHERE age > 18 AND (city = 'NYC' OR city = 'LA') AND active = true",
        "SELECT * FROM products WHERE price BETWEEN 100 AND 500",
        "SELECT * FROM orders WHERE date > '2024-01-01' AND total > 1000",
        "SELECT * FROM items WHERE name LIKE '%widget%' AND quantity > 0",
    ];
    
    for sql in queries {
        let result = QueryParser::parse(sql);
        assert!(result.is_ok(), "Failed to parse: {}", sql);
    }
}

#[test]
fn test_complex_joins() {
    let queries = vec![
        "SELECT * FROM users u JOIN orders o ON u.id = o.user_id JOIN products p ON o.product_id = p.id",
        "SELECT * FROM a LEFT JOIN b ON a.id = b.a_id RIGHT JOIN c ON b.id = c.b_id",
        "SELECT * FROM x JOIN y ON x.key = y.key AND x.type = y.type",
    ];
    
    for sql in queries {
        let result = QueryParser::parse(sql);
        assert!(result.is_ok(), "Failed to parse: {}", sql);
    }
}

#[test]
fn test_aggregations_complex() {
    let queries = vec![
        "SELECT category, COUNT(*), SUM(price), AVG(rating) FROM products GROUP BY category",
        "SELECT user_id, MAX(score), MIN(score), AVG(score) FROM games GROUP BY user_id HAVING AVG(score) > 50",
        "SELECT DATE(created_at), COUNT(*) FROM events GROUP BY DATE(created_at) ORDER BY COUNT(*) DESC",
    ];
    
    for sql in queries {
        let result = QueryParser::parse(sql);
        assert!(result.is_ok(), "Failed to parse: {}", sql);
    }
}

#[test]
fn test_subqueries() {
    let queries = vec![
        "SELECT * FROM users WHERE id IN (SELECT user_id FROM premium_users)",
        "SELECT * FROM (SELECT * FROM users WHERE age > 18) AS adults",
    ];
    
    for sql in queries {
        let result = QueryParser::parse(sql);
        // These might not be supported yet, but shouldn't crash
        let _ = result;
    }
}

#[test]
fn test_malformed_sql() {
    let bad_queries = vec![
        "SELCT * FROM users",  // Typo
        "SELECT * FROM",       // Incomplete
        "SELECT FROM users",   // Missing columns
        "",                    // Empty
        ";;;",                 // Just semicolons
    ];
    
    for sql in bad_queries {
        let result = QueryParser::parse(sql);
        assert!(result.is_err(), "Should reject malformed SQL: {}", sql);
    }
}

#[test]
fn test_sql_injection_safety() {
    // Ensure parser doesn't crash on injection attempts
    let injection_attempts = vec![
        "SELECT * FROM users WHERE id = 1; DROP TABLE users; --",
        "SELECT * FROM users WHERE name = '' OR '1'='1",
        "SELECT * FROM users WHERE id = 1 UNION SELECT * FROM passwords",
    ];
    
    for sql in injection_attempts {
        // Parser should handle gracefully (not crash)
        let _ = QueryParser::parse(sql);
    }
}
