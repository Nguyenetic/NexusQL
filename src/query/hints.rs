//! SQL Hint Parser - Engine Selection via Comments
//! PROPRIETARY - Best Practice from PostgreSQL/sqlparser-rs

/// Extract engine hint from SQL comment
/// 
/// Example: "SELECT * FROM docs /* ENGINE: colbert */"
/// Returns: Some("colbert")
pub fn extract_engine_hint(sql: &str) -> Option<String> {
    // Look for /* ENGINE: xxx */ pattern
    if let Some(start) = sql.find("/* ENGINE:") {
        let after_marker = &sql[start + 10..];
        if let Some(end) = after_marker.find("*/") {
            let engine_name = after_marker[..end].trim();
            return Some(engine_name.to_string());
        }
    }
    
    // Also support -- ENGINE: xxx (single-line comment)
    if let Some(start) = sql.find("-- ENGINE:") {
        let after_marker = &sql[start + 10..];
        let engine_name = after_marker
            .lines()
            .next()
            .unwrap_or("")
            .trim();
        if !engine_name.is_empty() {
            return Some(engine_name.to_string());
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_block_comment() {
        let sql = "SELECT * FROM docs /* ENGINE: colbert */";
        assert_eq!(extract_engine_hint(sql), Some("colbert".to_string()));
    }
    
    #[test]
    fn test_extract_line_comment() {
        let sql = "SELECT * FROM docs -- ENGINE: hdc";
        assert_eq!(extract_engine_hint(sql), Some("hdc".to_string()));
    }
    
    #[test]
    fn test_no_hint() {
        let sql = "SELECT * FROM docs";
        assert_eq!(extract_engine_hint(sql), None);
    }
}
