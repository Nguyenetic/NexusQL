//! Schema System & Validation Demo
//!
//! Demonstrates:
//! - Schema creation with typed columns
//! - Constraint enforcement (PRIMARY KEY, UNIQUE, NOT NULL, FOREIGN KEY)
//! - Data validation
//! - Type coercion
//! - Default values
//!
//! Based on PostgreSQL schema system from Context7 research

use nexusql::schema::{
    ColumnDef, Constraint, DataType, ReferentialAction, Schema, TableSchema, Validator,
};
use nexusql::types::{Row, Value};

fn main() {
    println!("=== NexusQL Schema System Demo ===\n");

    // Create a schema
    let mut app_schema = Schema::new("app".to_string(), "admin".to_string());

    // Demo 1: Create users table with constraints
    println!("📋 Demo 1: Creating Users Table with Constraints");
    let users_table = create_users_table();
    println!("  Table: {}", users_table.name);
    println!("  Columns: {}", users_table.columns.len());
    println!("  Constraints: {}", users_table.constraints.len());

    for col in &users_table.columns {
        println!(
            "    - {} ({:?}, nullable={})",
            col.name, col.data_type, col.nullable
        );
    }

    app_schema.add_table(users_table.clone()).unwrap();
    println!("  ✅ Table created successfully\n");

    // Demo 2: Valid insert
    println!("📋 Demo 2: Valid INSERT");
    let validator = Validator::new(users_table.clone());

    let valid_row = Row::new(
        vec![
            "id".to_string(),
            "username".to_string(),
            "email".to_string(),
            "age".to_string(),
        ],
        vec![
            Value::Integer(1),
            Value::Text("alice".to_string()),
            Value::Text("alice@example.com".to_string()),
            Value::Integer(25),
        ],
    );

    match validator.validate_insert(&valid_row, &[]) {
        Ok(validated) => {
            println!("  ✅ Insert validated successfully");
            println!("  User: {:?}", validated.values);
        }
        Err(e) => println!("  ❌ Validation failed: {}", e),
    }
    println!();

    // Demo 3: NOT NULL violation
    println!("📋 Demo 3: NOT NULL Violation");
    let null_row = Row::new(
        vec![
            "id".to_string(),
            "username".to_string(),
            "email".to_string(),
            "age".to_string(),
        ],
        vec![
            Value::Integer(2),
            Value::Null, // username is NOT NULL!
            Value::Text("bob@example.com".to_string()),
            Value::Integer(30),
        ],
    );

    match validator.validate_insert(&null_row, &[]) {
        Ok(_) => println!("  ❌ Should have failed"),
        Err(e) => println!("  ✅ Correctly rejected: {}", e),
    }
    println!();

    // Demo 4: PRIMARY KEY violation
    println!("📋 Demo 4: PRIMARY KEY Violation (Duplicate)");
    let existing_rows = vec![valid_row.clone()];

    let duplicate_id = Row::new(
        vec![
            "id".to_string(),
            "username".to_string(),
            "email".to_string(),
            "age".to_string(),
        ],
        vec![
            Value::Integer(1), // Same ID as existing!
            Value::Text("bob".to_string()),
            Value::Text("bob@example.com".to_string()),
            Value::Integer(30),
        ],
    );

    match validator.validate_insert(&duplicate_id, &existing_rows) {
        Ok(_) => println!("  ❌ Should have failed"),
        Err(e) => println!("  ✅ Correctly rejected: {}", e),
    }
    println!();

    // Demo 5: UNIQUE constraint violation
    println!("📋 Demo 5: UNIQUE Constraint Violation");
    let duplicate_email = Row::new(
        vec![
            "id".to_string(),
            "username".to_string(),
            "email".to_string(),
            "age".to_string(),
        ],
        vec![
            Value::Integer(2),
            Value::Text("bob".to_string()),
            Value::Text("alice@example.com".to_string()), // Same email as alice!
            Value::Integer(30),
        ],
    );

    match validator.validate_insert(&duplicate_email, &existing_rows) {
        Ok(_) => println!("  ❌ Should have failed"),
        Err(e) => println!("  ✅ Correctly rejected: {}", e),
    }
    println!();

    // Demo 6: Type coercion
    println!("📋 Demo 6: Data Type Coercion");
    println!("  Testing VARCHAR(50) length enforcement:");

    let short_username = Value::Text("alice".to_string());
    match DataType::Varchar(50).coerce(short_username.clone()) {
        Ok(_) => println!("    ✅ Short username accepted: {:?}", short_username),
        Err(e) => println!("    ❌ Failed: {}", e),
    }

    let long_username = Value::Text("a".repeat(100));
    match DataType::Varchar(50).coerce(long_username.clone()) {
        Ok(_) => println!("    ❌ Should have rejected long username"),
        Err(e) => println!("    ✅ Correctly rejected: {}", e),
    }
    println!();

    // Demo 7: Default values
    println!("📋 Demo 7: Default Values");
    let mut posts_table = TableSchema::new("posts".to_string());

    posts_table
        .add_column(ColumnDef::new("id".to_string(), DataType::Integer).primary_key())
        .unwrap();

    posts_table
        .add_column(ColumnDef::new("title".to_string(), DataType::Text).not_null())
        .unwrap();

    posts_table
        .add_column(
            ColumnDef::new("status".to_string(), DataType::Text)
                .default(Value::Text("draft".to_string())),
        )
        .unwrap();

    posts_table
        .add_constraint(Constraint::PrimaryKey {
            name: "posts_pkey".to_string(),
            columns: vec!["id".to_string()],
        })
        .unwrap();

    let posts_validator = Validator::new(posts_table);

    let row_with_null_status = Row::new(
        vec!["id".to_string(), "title".to_string(), "status".to_string()],
        vec![
            Value::Integer(1),
            Value::Text("My First Post".to_string()),
            Value::Null, // Will use default value
        ],
    );

    match posts_validator.validate_insert(&row_with_null_status, &[]) {
        Ok(validated) => {
            println!("  ✅ Default value applied");
            println!("  Status: {:?}", validated.values[2]);
        }
        Err(e) => println!("  ❌ Failed: {}", e),
    }
    println!();

    // Demo 8: Composite PRIMARY KEY
    println!("📋 Demo 8: Composite PRIMARY KEY");
    let mut user_roles = TableSchema::new("user_roles".to_string());

    user_roles
        .add_column(ColumnDef::new("user_id".to_string(), DataType::Integer).not_null())
        .unwrap();

    user_roles
        .add_column(ColumnDef::new("role_id".to_string(), DataType::Integer).not_null())
        .unwrap();

    user_roles
        .add_constraint(Constraint::PrimaryKey {
            name: "user_roles_pkey".to_string(),
            columns: vec!["user_id".to_string(), "role_id".to_string()],
        })
        .unwrap();

    println!("  Table: user_roles");
    println!("  PRIMARY KEY: (user_id, role_id)");
    println!("  ✅ Composite key created\n");

    // Demo 9: FOREIGN KEY definition
    println!("📋 Demo 9: FOREIGN KEY Constraint");
    let mut orders_table = TableSchema::new("orders".to_string());

    orders_table
        .add_column(ColumnDef::new("id".to_string(), DataType::Integer).primary_key())
        .unwrap();

    orders_table
        .add_column(ColumnDef::new("user_id".to_string(), DataType::Integer).not_null())
        .unwrap();

    orders_table
        .add_column(ColumnDef::new("total".to_string(), DataType::Float).not_null())
        .unwrap();

    orders_table
        .add_constraint(Constraint::PrimaryKey {
            name: "orders_pkey".to_string(),
            columns: vec!["id".to_string()],
        })
        .unwrap();

    orders_table
        .add_constraint(Constraint::ForeignKey {
            name: "orders_user_id_fkey".to_string(),
            columns: vec!["user_id".to_string()],
            ref_table: "users".to_string(),
            ref_columns: vec!["id".to_string()],
            on_delete: ReferentialAction::Cascade, // DELETE user -> DELETE orders
            on_update: ReferentialAction::NoAction,
        })
        .unwrap();

    println!("  Table: orders");
    println!("  FOREIGN KEY: user_id REFERENCES users(id)");
    println!("  ON DELETE: CASCADE");
    println!("  ✅ Foreign key constraint created\n");

    // Demo 10: Schema information
    println!("📋 Demo 10: Schema Information");
    println!("  Schema: {}", app_schema.name);
    println!("  Owner: {}", app_schema.owner);
    println!("  Tables: {}", app_schema.tables.len());

    for (table_name, table) in &app_schema.tables {
        println!("\n  Table: {}", table_name);
        println!("    Columns:");
        for col in &table.columns {
            println!(
                "      - {} {:?} {}",
                col.name,
                col.data_type,
                if col.nullable { "NULL" } else { "NOT NULL" }
            );
        }

        if !table.constraints.is_empty() {
            println!("    Constraints:");
            for constraint in &table.constraints {
                match constraint {
                    Constraint::PrimaryKey { name, columns } => {
                        println!("      - PRIMARY KEY {} ({})", name, columns.join(", "));
                    }
                    Constraint::Unique { name, columns, .. } => {
                        println!("      - UNIQUE {} ({})", name, columns.join(", "));
                    }
                    Constraint::ForeignKey {
                        name,
                        columns,
                        ref_table,
                        ref_columns,
                        ..
                    } => {
                        println!(
                            "      - FOREIGN KEY {} ({}) REFERENCES {}({})",
                            name,
                            columns.join(", "),
                            ref_table,
                            ref_columns.join(", ")
                        );
                    }
                    Constraint::Check {
                        name, expression, ..
                    } => {
                        println!("      - CHECK {} ({})", name, expression);
                    }
                }
            }
        }
    }

    println!("\n✅ Schema System Demo Complete!");
    println!("\n📊 Summary:");
    println!("  - Type-safe columns with data type enforcement");
    println!("  - PRIMARY KEY, UNIQUE, NOT NULL validation");
    println!("  - FOREIGN KEY constraints with CASCADE");
    println!("  - Default values support");
    println!("  - Type coercion (e.g., VARCHAR length limits)");
    println!("  - Composite keys support");
    println!("\n🔒 PostgreSQL-Compatible Schema System!");
}

fn create_users_table() -> TableSchema {
    let mut table = TableSchema::new("users".to_string());

    // id INTEGER PRIMARY KEY
    table
        .add_column(ColumnDef::new("id".to_string(), DataType::Integer).primary_key())
        .unwrap();

    // username VARCHAR(50) UNIQUE NOT NULL
    table
        .add_column(
            ColumnDef::new("username".to_string(), DataType::Varchar(50))
                .not_null()
                .unique(),
        )
        .unwrap();

    // email VARCHAR(100) UNIQUE NOT NULL
    table
        .add_column(
            ColumnDef::new("email".to_string(), DataType::Varchar(100))
                .not_null()
                .unique(),
        )
        .unwrap();

    // age INTEGER
    table
        .add_column(ColumnDef::new("age".to_string(), DataType::Integer))
        .unwrap();

    // Add PRIMARY KEY constraint
    table
        .add_constraint(Constraint::PrimaryKey {
            name: "users_pkey".to_string(),
            columns: vec!["id".to_string()],
        })
        .unwrap();

    // Add UNIQUE constraints
    table
        .add_constraint(Constraint::Unique {
            name: "users_username_key".to_string(),
            columns: vec!["username".to_string()],
            nulls_distinct: true,
        })
        .unwrap();

    table
        .add_constraint(Constraint::Unique {
            name: "users_email_key".to_string(),
            columns: vec!["email".to_string()],
            nulls_distinct: true,
        })
        .unwrap();

    table
}
