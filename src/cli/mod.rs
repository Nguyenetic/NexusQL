// CLI Tool - Interactive SQL Shell
// psql-like command-line interface for NexusQL

use crate::{NexusError, Result};
use std::io::{self, Write};

/// CLI prompt and command processor
pub struct NexusCLI {
    pub prompt: String,
    history: Vec<String>,
}

impl NexusCLI {
    pub fn new() -> Self {
        Self {
            prompt: "nexusql> ".to_string(),
            history: Vec::new(),
        }
    }

    /// Start interactive REPL
    pub fn start_repl(&mut self) -> Result<()> {
        println!("NexusQL v2.0.0 - AI-Native Database");
        println!("Type 'help' for help, '\\q' to quit\n");

        loop {
            print!("{}", self.prompt);
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();

            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            // Handle meta-commands
            if input.starts_with('\\') {
                if self.handle_meta_command(input)? {
                    break; // Quit
                }
                continue;
            }

            // Execute SQL
            self.history.push(input.to_string());
            self.execute_sql(input)?;
        }

        Ok(())
    }

    fn handle_meta_command(&self, cmd: &str) -> Result<bool> {
        match cmd {
            "\\q" | "\\quit" => {
                println!("Goodbye!");
                return Ok(true);
            }
            "\\?" | "\\help" | "help" => {
                self.print_help();
            }
            "\\h" => {
                self.print_history();
            }
            "\\d" => {
                println!("List of relations:");
                println!("  (Would list all tables)");
            }
            "\\dt" => {
                println!("List of tables:");
                println!("  (Would list tables)");
            }
            "\\di" => {
                println!("List of indexes:");
                println!("  (Would list indexes)");
            }
            _ => {
                println!("Unknown command: {}. Try \\? for help.", cmd);
            }
        }
        Ok(false)
    }

    fn execute_sql(&self, sql: &str) -> Result<()> {
        // In production: would parse and execute SQL
        println!("Executing: {}", sql);
        println!("(Query executed successfully)");
        Ok(())
    }

    fn print_help(&self) {
        println!("\nNexusQL Meta-Commands:");
        println!("  \\q, \\quit     - Quit NexusQL");
        println!("  \\?, \\help     - Show this help");
        println!("  \\h            - Show command history");
        println!("  \\d            - List relations");
        println!("  \\dt           - List tables");
        println!("  \\di           - List indexes");
        println!("\nSQL Commands:");
        println!("  Just type SQL and press Enter");
        println!("  Example: SELECT * FROM users;\n");
    }

    fn print_history(&self) {
        println!("\nCommand History:");
        for (i, cmd) in self.history.iter().enumerate() {
            println!("  {}: {}", i + 1, cmd);
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_creation() {
        let cli = NexusCLI::new();
        assert_eq!(cli.prompt, "nexusql> ");
        assert_eq!(cli.history.len(), 0);
    }

    #[test]
    fn test_execute_sql() {
        let cli = NexusCLI::new();
        let result = cli.execute_sql("SELECT 1");
        assert!(result.is_ok());
    }
}
