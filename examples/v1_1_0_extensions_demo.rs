// NexusQL v1.1.0 - Extensions & Plugins Demo
// Demonstrates extension framework, FDW, custom types, and custom operators

use nexusql::extensions::{
    CustomDataType, CustomOperator, ExtensionRegistry, ForeignDataWrapper, ForeignServer,
    ForeignTable,
};

fn main() {
    println!("🔌 NexusQL v1.1.0 - Extensions & Plugins Demo");
    println!("============================================\n");

    demo_extension_loading();
    demo_custom_data_types();
    demo_custom_operators();
    demo_foreign_data_wrappers();

    println!("\n✅ All v1.1.0 features demonstrated!");
    println!("\n🎉 NexusQL v1.1.0 - Extensible & Pluggable!");
}

fn demo_extension_loading() {
    println!("📦 Demo 1: Extension Loading (CREATE EXTENSION)");
    println!("------------------------------------------------\n");

    let registry = ExtensionRegistry::new();

    // Create extensions
    println!("Creating extensions...");
    registry
        .create_extension("pg_trgm".to_string(), "1.6".to_string())
        .unwrap();
    println!("  ✓ CREATE EXTENSION pg_trgm VERSION 1.6");

    registry
        .create_extension("postgis".to_string(), "3.4.0".to_string())
        .unwrap();
    println!("  ✓ CREATE EXTENSION postgis VERSION 3.4.0");

    registry
        .create_extension("uuid-ossp".to_string(), "1.1".to_string())
        .unwrap();
    println!("  ✓ CREATE EXTENSION uuid-ossp VERSION 1.1\n");

    // List installed extensions
    let extensions = registry.list_extensions();
    println!("Installed extensions ({}):", extensions.len());
    for ext in &extensions {
        println!("  • {} (v{}): {}", ext.name, ext.version, ext.description);
    }

    // Drop extension
    println!("\nDropping extension...");
    registry.drop_extension("uuid-ossp", false).unwrap();
    println!("  ✓ DROP EXTENSION uuid-ossp");
    println!(
        "  Remaining: {} extensions",
        registry.list_extensions().len()
    );
}

fn demo_custom_data_types() {
    println!("\n\n🎨 Demo 2: Custom Data Types");
    println!("-----------------------------\n");

    let registry = ExtensionRegistry::new();

    // Define custom 'complex' type (complex numbers: a + bi)
    println!("Creating custom type: complex");
    let complex_type = CustomDataType::new("complex".to_string())
        .with_length(16) // 2 doubles = 16 bytes
        .with_alignment("double".to_string());

    registry.register_type(complex_type).unwrap();
    println!("  ✓ CREATE TYPE complex (internallength = 16, alignment = double)\n");

    // Define custom 'pair' type
    println!("Creating custom type: pair");
    let pair_type = CustomDataType::new("pair".to_string()).with_alignment("int4".to_string());

    registry.register_type(pair_type).unwrap();
    println!("  ✓ CREATE TYPE pair AS (k text, v text)\n");

    // Retrieve and display
    println!("Registered custom types:");
    if let Some(complex) = registry.get_type("complex") {
        println!(
            "  • {}: length={}, alignment={}",
            complex.name, complex.internal_length, complex.alignment
        );
    }
    if let Some(pair) = registry.get_type("pair") {
        println!(
            "  • {}: variable length, alignment={}",
            pair.name, pair.alignment
        );
    }
}

fn demo_custom_operators() {
    println!("\n\n⚙️  Demo 3: Custom Operators");
    println!("----------------------------\n");

    let registry = ExtensionRegistry::new();

    // Define custom operator: ~> (creates a pair)
    println!("Creating custom operator: ~>");
    let pair_op = CustomOperator::new(
        "~>".to_string(),
        "text".to_string(),
        "text".to_string(),
        "pair".to_string(),
    );

    registry.register_operator(pair_op).unwrap();
    println!("  ✓ CREATE OPERATOR ~> (LEFTARG = text, RIGHTARG = text)\n");

    // Define custom operator: @+ (complex addition)
    println!("Creating custom operator: @+ (complex addition)");
    let complex_add = CustomOperator::new(
        "@+".to_string(),
        "complex".to_string(),
        "complex".to_string(),
        "complex".to_string(),
    )
    .with_commutator("@+".to_string());

    registry.register_operator(complex_add).unwrap();
    println!("  ✓ CREATE OPERATOR @+ (complex + complex → complex)");
    println!("    COMMUTATOR = @+ (addition is commutative)\n");

    println!("Usage examples:");
    println!("  'key' ~> 'value'  → Creates pair('key', 'value')");
    println!("  (3,4) @+ (1,2)    → Complex addition: (4,6)");
}

fn demo_foreign_data_wrappers() {
    println!("\n\n🌐 Demo 4: Foreign Data Wrappers (FDW)");
    println!("---------------------------------------\n");

    let registry = ExtensionRegistry::new();

    // Register file_fdw
    println!("Creating Foreign Data Wrapper...");
    let file_fdw = ForeignDataWrapper::new("file_fdw".to_string(), "file_fdw_handler".to_string())
        .with_validator("file_fdw_validator".to_string());

    registry
        .register_fdw("file_fdw".to_string(), file_fdw)
        .unwrap();
    println!("  ✓ CREATE FOREIGN DATA WRAPPER file_fdw");
    println!("    HANDLER file_fdw_handler");
    println!("    VALIDATOR file_fdw_validator\n");

    // Create foreign server
    println!("Creating Foreign Server...");
    let csv_server = ForeignServer::new("csv_server".to_string(), "file_fdw".to_string())
        .with_option("directory".to_string(), "/data/csv".to_string());

    println!("  ✓ CREATE SERVER csv_server");
    println!("    FOREIGN DATA WRAPPER file_fdw");
    println!("    OPTIONS (directory '/data/csv')\n");

    // Create foreign table
    println!("Creating Foreign Table...");
    let logs_table = ForeignTable::new(
        "access_logs".to_string(),
        "csv_server".to_string(),
        vec![
            "timestamp".to_string(),
            "ip_address".to_string(),
            "request".to_string(),
            "status_code".to_string(),
        ],
    );

    println!("  ✓ CREATE FOREIGN TABLE access_logs (");
    println!("      timestamp text,");
    println!("      ip_address text,");
    println!("      request text,");
    println!("      status_code integer");
    println!("    ) SERVER csv_server\n");

    println!("Usage:");
    println!("  SELECT * FROM access_logs WHERE status_code = 404;");
    println!("\n  💡 Query external data sources as native tables!");
    println!("  💡 Supports CSV files, PostgreSQL, REST APIs, and more!");
}
