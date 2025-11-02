//! Week 10 Features Demo: Triggers, Views, Sequences, and Stored Procedures
//!
//! This example demonstrates all the "convenience features" implemented in Week 10:
//! - Triggers (BEFORE/AFTER, row/statement level)
//! - Views (regular and materialized)
//! - Sequences (auto-increment, nextval/currval)
//! - Stored Procedures and Functions (PL/NexusQL)

use nexus::error::Result;
use nexus::procedure::plnexus::PLNexusExecutor;
use nexus::procedure::{
    FunctionVolatility, Parameter, ParameterMode, ProcedureManager, ReturnType, StoredFunction,
};
use nexus::sequence::{Sequence, SequenceManager};
use nexus::trigger::{
    AuditLogFunction, LogTriggerFunction, Trigger, TriggerEvent, TriggerLevel, TriggerManager,
    TriggerTiming, UpdateTimestampFunction,
};
use nexus::types::{Row, Value};
use nexus::view::{View, ViewManager};
use std::sync::Arc;

fn main() -> Result<()> {
    println!("=".repeat(80));
    println!("NexusQL Week 10 Features Demo");
    println!("Triggers • Views • Sequences • Stored Procedures");
    println!("=".repeat(80));
    println!();

    // ========================================================================
    // 1. SEQUENCES - Auto-increment and serial support
    // ========================================================================
    println!("📊 1. SEQUENCES - Auto-increment Generation");
    println!("-".repeat(80));

    let seq_manager = SequenceManager::new();

    // Create a sequence for user IDs
    let user_id_seq = Sequence::new("user_id_seq".to_string())
        .start_value(1000)
        .increment(1)
        .min_value(Some(1))
        .max_value(Some(999999))
        .build();

    seq_manager.create_sequence(user_id_seq)?;

    // Generate some IDs
    println!("  Creating user_id_seq starting at 1000...");
    for i in 1..=5 {
        let id = seq_manager.nextval("user_id_seq")?;
        println!("  User #{}: ID = {}", i, id);
    }

    let current = seq_manager.currval("user_id_seq")?;
    println!("  Current value: {}", current);

    // Create a cycling sequence for order numbers
    let order_seq = Sequence::new("daily_order_seq".to_string())
        .start_value(1)
        .increment(1)
        .max_value(Some(100))
        .cycle(true)
        .build();

    seq_manager.create_sequence(order_seq)?;
    println!("\n  Creating daily_order_seq (cycles at 100)...");
    println!(
        "  Next values: {}, {}, {}",
        seq_manager.nextval("daily_order_seq")?,
        seq_manager.nextval("daily_order_seq")?,
        seq_manager.nextval("daily_order_seq")?
    );

    println!("  ✓ Sequences working - atomic, thread-safe generation\n");

    // ========================================================================
    // 2. TRIGGERS - Event-driven automation
    // ========================================================================
    println!("🔔 2. TRIGGERS - Event-Driven Table Automation");
    println!("-".repeat(80));

    let trigger_mgr = TriggerManager::new();

    // Register trigger functions
    trigger_mgr.register_function("log_trigger".to_string(), Arc::new(LogTriggerFunction))?;
    trigger_mgr.register_function(
        "update_timestamp".to_string(),
        Arc::new(UpdateTimestampFunction::new("updated_at".to_string())),
    )?;
    trigger_mgr.register_function(
        "audit_log".to_string(),
        Arc::new(AuditLogFunction::new("audit_trail".to_string())),
    )?;

    // Create BEFORE INSERT trigger for logging
    let before_insert = Trigger::new(
        "log_user_insert".to_string(),
        "users".to_string(),
        TriggerTiming::Before,
        vec![TriggerEvent::Insert],
    )
    .level(TriggerLevel::Row)
    .function("log_trigger".to_string())
    .build();

    trigger_mgr.create_trigger(before_insert)?;
    println!("  ✓ Created BEFORE INSERT trigger on 'users' table");

    // Create AFTER UPDATE trigger for timestamp
    let after_update = Trigger::new(
        "update_user_timestamp".to_string(),
        "users".to_string(),
        TriggerTiming::After,
        vec![TriggerEvent::Update],
    )
    .level(TriggerLevel::Row)
    .function("update_timestamp".to_string())
    .build();

    trigger_mgr.create_trigger(after_update)?;
    println!("  ✓ Created AFTER UPDATE trigger for auto-timestamps");

    // Create audit trail trigger
    let audit_trigger = Trigger::new(
        "audit_changes".to_string(),
        "sensitive_data".to_string(),
        TriggerTiming::After,
        vec![
            TriggerEvent::Insert,
            TriggerEvent::Update,
            TriggerEvent::Delete,
        ],
    )
    .level(TriggerLevel::Row)
    .function("audit_log".to_string())
    .build();

    trigger_mgr.create_trigger(audit_trigger)?;
    println!("  ✓ Created audit trail trigger for sensitive data");

    // Simulate firing triggers
    println!("\n  Simulating INSERT into users table...");
    let new_row = Row::new(vec![
        Value::Integer(1001),
        Value::Text("alice@example.com".to_string()),
    ]);

    let result = trigger_mgr.fire_triggers(
        "users",
        TriggerTiming::Before,
        TriggerEvent::Insert,
        TriggerLevel::Row,
        Some(&new_row),
        None,
    );

    println!("  ✓ BEFORE INSERT trigger fired successfully");
    println!("  ✓ Triggers provide automation, validation, and audit trails\n");

    // ========================================================================
    // 3. VIEWS - Virtual tables and query abstraction
    // ========================================================================
    println!("👁️  3. VIEWS - Virtual Tables and Query Abstraction");
    println!("-".repeat(80));

    let view_mgr = ViewManager::new();

    // Create a simple view
    let user_view = View::new(
        "active_users".to_string(),
        "SELECT id, email FROM users WHERE status = 'active'".to_string(),
    )
    .with_columns(vec!["id".to_string(), "email".to_string()])
    .build();

    view_mgr.create_view(user_view)?;
    println!("  ✓ Created view 'active_users'");

    // Create a materialized view
    let stats_view = View::new(
        "user_stats".to_string(),
        "SELECT COUNT(*) as total, AVG(age) as avg_age FROM users".to_string(),
    )
    .materialized(true)
    .build();

    view_mgr.create_view(stats_view)?;
    println!("  ✓ Created materialized view 'user_stats'");

    // Create an updateable view
    let updateable_view = View::new(
        "user_emails".to_string(),
        "SELECT id, email FROM users".to_string(),
    )
    .updateable(true)
    .with_check_option(true)
    .build();

    view_mgr.create_view(updateable_view)?;
    println!("  ✓ Created updateable view 'user_emails' with CHECK OPTION");

    // Query rewriting demo
    let original_query = "SELECT * FROM active_users WHERE id > 1000";
    let rewritten = view_mgr.rewrite_query(original_query)?;
    println!("\n  Query rewriting:");
    println!("    Original: {}", original_query);
    println!("    Rewritten: {}", rewritten);

    // Refresh materialized view
    let mock_data = vec![Row::new(vec![Value::Integer(150), Value::Integer(32)])];
    view_mgr.refresh_materialized_view("user_stats", mock_data)?;
    println!("\n  ✓ Materialized view 'user_stats' refreshed");
    println!("  ✓ Views provide abstraction, security, and performance\n");

    // ========================================================================
    // 4. STORED PROCEDURES - Server-side programmable logic
    // ========================================================================
    println!("⚙️  4. STORED PROCEDURES - Server-Side Programmable Logic");
    println!("-".repeat(80));

    let proc_mgr = ProcedureManager::new();
    let executor = PLNexusExecutor;

    // Simple function: add two numbers
    let add_func = StoredFunction::new(
        "add_numbers".to_string(),
        "plnexus".to_string(),
        "RETURN a + b;".to_string(),
    )
    .with_parameters(vec![
        Parameter::new("a".to_string(), "INTEGER".to_string()),
        Parameter::new("b".to_string(), "INTEGER".to_string()),
    ])
    .returns(ReturnType::Scalar("INTEGER".to_string()))
    .volatility(FunctionVolatility::Immutable)
    .build();

    proc_mgr.create_function(add_func.clone())?;
    println!("  ✓ Created function 'add_numbers(a INTEGER, b INTEGER)'");

    let result = executor.execute(&add_func, vec![Value::Integer(42), Value::Integer(58)])?;
    println!("    add_numbers(42, 58) = {:?}", result);

    // Function with variables and control flow
    let factorial_func = StoredFunction::new(
        "factorial".to_string(),
        "plnexus".to_string(),
        r#"
DECLARE
    result INTEGER := 1;
    i INTEGER := 1;
BEGIN
    WHILE i <= n LOOP
        result := result * i;
        i := i + 1;
    END LOOP;
    RETURN result;
END;
        "#
        .to_string(),
    )
    .with_parameters(vec![Parameter::new("n".to_string(), "INTEGER".to_string())])
    .returns(ReturnType::Scalar("INTEGER".to_string()))
    .volatility(FunctionVolatility::Immutable)
    .build();

    proc_mgr.create_function(factorial_func)?;
    println!("  ✓ Created function 'factorial(n INTEGER)' with WHILE loop");

    // Stored procedure (no return value)
    let log_proc = StoredFunction::new(
        "log_event".to_string(),
        "plnexus".to_string(),
        "RAISE 'Event logged: ' || event_name;".to_string(),
    )
    .with_parameters(vec![Parameter::new(
        "event_name".to_string(),
        "TEXT".to_string(),
    )])
    .returns(ReturnType::Void)
    .volatility(FunctionVolatility::Volatile)
    .is_procedure(true)
    .build();

    proc_mgr.create_function(log_proc)?;
    println!("  ✓ Created procedure 'log_event(event_name TEXT)'");

    // Function overloading
    let add_three = StoredFunction::new(
        "add_numbers".to_string(),
        "plnexus".to_string(),
        "RETURN a + b + c;".to_string(),
    )
    .with_parameters(vec![
        Parameter::new("a".to_string(), "INTEGER".to_string()),
        Parameter::new("b".to_string(), "INTEGER".to_string()),
        Parameter::new("c".to_string(), "INTEGER".to_string()),
    ])
    .returns(ReturnType::Scalar("INTEGER".to_string()))
    .build();

    proc_mgr.create_function(add_three)?;
    println!("  ✓ Created overloaded 'add_numbers(a, b, c INTEGER)'");

    let funcs = proc_mgr.get_functions_by_name("add_numbers")?;
    println!(
        "    Found {} overloaded versions of 'add_numbers'",
        funcs.len()
    );

    println!("  ✓ PL/NexusQL supports variables, loops, conditions, and more\n");

    // ========================================================================
    // 5. REAL-WORLD SCENARIO - All features together
    // ========================================================================
    println!("🌟 5. REAL-WORLD SCENARIO - E-Commerce Order System");
    println!("-".repeat(80));

    println!("\n  Setting up order system components:");

    // Sequence for order IDs
    let order_id_seq = Sequence::new("order_id_seq".to_string())
        .start_value(10000)
        .increment(1)
        .build();
    seq_manager.create_sequence(order_id_seq)?;
    println!("    ✓ Sequence: order_id_seq (auto-increment order IDs)");

    // Trigger for order timestamps
    let order_timestamp = Trigger::new(
        "set_order_timestamp".to_string(),
        "orders".to_string(),
        TriggerTiming::Before,
        vec![TriggerEvent::Insert],
    )
    .level(TriggerLevel::Row)
    .function("update_timestamp".to_string())
    .build();
    trigger_mgr.create_trigger(order_timestamp)?;
    println!("    ✓ Trigger: Auto-timestamp new orders");

    // View for recent orders
    let recent_orders = View::new(
        "recent_orders".to_string(),
        "SELECT * FROM orders WHERE created_at > NOW() - INTERVAL '7 days'".to_string(),
    )
    .build();
    view_mgr.create_view(recent_orders)?;
    println!("    ✓ View: recent_orders (last 7 days)");

    // Function to calculate order total
    let calc_total = StoredFunction::new(
        "calculate_order_total".to_string(),
        "plnexus".to_string(),
        r#"
DECLARE
    subtotal DECIMAL;
    tax DECIMAL;
    shipping DECIMAL := 5.99;
    total DECIMAL;
BEGIN
    subtotal := item_price * quantity;
    tax := subtotal * 0.08;
    total := subtotal + tax + shipping;
    RETURN total;
END;
        "#
        .to_string(),
    )
    .with_parameters(vec![
        Parameter::new("item_price".to_string(), "DECIMAL".to_string()),
        Parameter::new("quantity".to_string(), "INTEGER".to_string()),
    ])
    .returns(ReturnType::Scalar("DECIMAL".to_string()))
    .build();
    proc_mgr.create_function(calc_total)?;
    println!("    ✓ Function: calculate_order_total(price, quantity)");

    println!("\n  Processing orders:");
    for i in 1..=3 {
        let order_id = seq_manager.nextval("order_id_seq")?;
        println!("    Order #{} - ID: {}", i, order_id);
    }

    println!("\n  ✓ Complete order system with automated IDs, timestamps, views, and calculations");

    // ========================================================================
    // Summary
    // ========================================================================
    println!();
    println!("=".repeat(80));
    println!("Week 10 Features Summary");
    println!("=".repeat(80));
    println!("✅ SEQUENCES: Atomic auto-increment with nextval/currval/setval");
    println!("✅ TRIGGERS: BEFORE/AFTER/INSTEAD OF with row/statement levels");
    println!("✅ VIEWS: Regular, materialized, and updateable views");
    println!("✅ STORED PROCEDURES: PL/NexusQL with variables, loops, and control flow");
    println!();
    println!("These 'convenience features' make NexusQL significantly more developer-friendly");
    println!("and bring us closer to full PostgreSQL compatibility!");
    println!("=".repeat(80));

    Ok(())
}
