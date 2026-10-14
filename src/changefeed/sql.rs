use super::{ChangefeedStatement, FeedKind};
use crate::query::parser::NexusQLDialect;
use crate::{NexusError, Result};
use sqlparser::tokenizer::{Token, Tokenizer};

/// Token-based parsing preserves quoted identifiers and ignores SQL comments.
pub fn parse_statement(sql: &str) -> Result<Option<ChangefeedStatement>> {
    let upper = sql.to_ascii_uppercase();
    if !upper.contains("CHANGEFEED") && !upper.contains("GEOFENCE") {
        return Ok(None);
    }
    let tokens = Tokenizer::new(&NexusQLDialect, sql)
        .tokenize()
        .map_err(|e| NexusError::ParseError(e.to_string()))?;
    let mut p = Tokens {
        tokens: tokens
            .into_iter()
            .filter(|t| !matches!(t, Token::Whitespace(_)))
            .collect(),
        pos: 0,
    };
    let create = p.word_at(0, "CREATE");
    let drop = p.word_at(0, "DROP");
    let feed = p.word_at(1, "CHANGEFEED");
    let fence = p.word_at(1, "GEOFENCE");
    if !(create && (feed || fence) || drop && feed) {
        return Ok(None);
    }
    p.pos = 2;
    let name = p.identifier()?;
    let command = if drop {
        ChangefeedStatement::Drop { name }
    } else {
        p.keyword("ON")?;
        let table = p.identifier()?;
        let kind = if fence {
            let geometry = p.column()?;
            p.keyword("ZONES")?;
            let zones = p.identifier()?;
            let zone_geometry = p.column()?;
            FeedKind::Geofence {
                table,
                geometry,
                zones,
                zone_geometry,
            }
        } else {
            let mut tables = vec![table];
            while p.consume(&Token::Comma) {
                tables.push(p.identifier()?);
            }
            FeedKind::Rows { tables }
        };
        ChangefeedStatement::Create { name, kind }
    };
    p.consume(&Token::SemiColon);
    if p.pos != p.tokens.len() {
        return Err(p.error("end of statement"));
    }
    Ok(Some(command))
}

struct Tokens {
    tokens: Vec<Token>,
    pos: usize,
}

impl Tokens {
    fn word_at(&self, pos: usize, expected: &str) -> bool {
        matches!(self.tokens.get(pos), Some(Token::Word(w))
            if w.quote_style.is_none() && w.value.eq_ignore_ascii_case(expected))
    }

    fn error(&self, expected: &str) -> NexusError {
        NexusError::ParseError(format!(
            "Expected {expected} in changefeed command, found {:?}",
            self.tokens.get(self.pos)
        ))
    }

    fn keyword(&mut self, expected: &str) -> Result<()> {
        if !self.word_at(self.pos, expected) {
            return Err(self.error(expected));
        }
        self.pos += 1;
        Ok(())
    }

    fn identifier(&mut self) -> Result<String> {
        let Some(Token::Word(w)) = self.tokens.get(self.pos) else {
            return Err(self.error("identifier"));
        };
        let mut name = w.value.clone();
        self.pos += 1;
        if self.consume(&Token::Period) {
            name.push('.');
            name.push_str(&self.identifier()?);
        }
        Ok(name)
    }

    fn consume(&mut self, token: &Token) -> bool {
        if self.tokens.get(self.pos) == Some(token) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn column(&mut self) -> Result<String> {
        if !self.consume(&Token::LParen) {
            return Err(self.error("("));
        }
        let name = self.identifier()?;
        if !self.consume(&Token::RParen) {
            return Err(self.error(")"));
        }
        Ok(name)
    }
}
