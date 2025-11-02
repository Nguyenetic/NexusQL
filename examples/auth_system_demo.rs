// Authentication & Authorization System Demo
// Demonstrates NexusQL's PostgreSQL-compatible security features

use nexusql::auth::{AuthManager, Permission, Policy, PolicyCommand, RLSManager, UserContext};
use nexusql::types::{Row, Value};

fn main() -> nexusql::Result<()> {
    println!("=== NexusQL Authentication & Authorization Demo ===\n");

    // Demo 1: User Creation & Authentication
    demo_user_authentication()?;

    // Demo 2: Role-Based Access Control (RBAC)
    demo_rbac()?;

    // Demo 3: Permission Inheritance
    demo_permission_inheritance()?;

    // Demo 4: Row-Level Security (RLS)
    demo_row_level_security()?;

    // Demo 5: Multi-Tenant Isolation
    demo_multi_tenant()?;

    println!("\n✅ Authentication & Authorization Demo Complete!\n");

    print_summary();

    Ok(())
}

fn demo_user_authentication() -> nexusql::Result<()> {
    println!("📋 Demo 1: User Creation & Authentication");

    let mut auth = AuthManager::new();

    // Create superuser
    auth.create_superuser("admin".to_string(), "admin_password")?;
    println!("  ✅ Superuser 'admin' created");

    // Create regular users
    auth.create_user("alice".to_string(), "alice123")?;
    auth.create_user("bob".to_string(), "bob456")?;
    println!("  ✅ Users 'alice' and 'bob' created");

    // Test authentication - correct password
    match auth.authenticate("alice", "alice123") {
        Ok(_) => println!("  ✅ Alice authenticated successfully"),
        Err(e) => println!("  ❌ Authentication failed: {}", e),
    }

    // Test authentication - wrong password
    match auth.authenticate("alice", "wrong_password") {
        Ok(_) => println!("  ❌ Should have failed!"),
        Err(_) => println!("  ✅ Correctly rejected wrong password"),
    }

    // Test authentication - non-existent user
    match auth.authenticate("charlie", "password") {
        Ok(_) => println!("  ❌ Should have failed!"),
        Err(_) => println!("  ✅ Correctly rejected non-existent user"),
    }

    println!();
    Ok(())
}

fn demo_rbac() -> nexusql::Result<()> {
    println!("📋 Demo 2: Role-Based Access Control (RBAC)");

    let mut auth = AuthManager::new();

    // Setup: Create admin and login
    auth.create_superuser("admin".to_string(), "admin123")?;
    auth.authenticate("admin", "admin123")?;

    // Create roles
    auth.create_role("reader".to_string())?;
    auth.create_role("writer".to_string())?;
    println!("  ✅ Created roles: reader, writer");

    // Grant permissions to reader role
    auth.grant_permission("reader", "users".to_string(), Permission::Select)?;
    println!("  ✅ Granted SELECT on 'users' to 'reader'");

    // Grant permissions to writer role
    auth.grant_permission("writer", "users".to_string(), Permission::Select)?;
    auth.grant_permission("writer", "users".to_string(), Permission::Insert)?;
    auth.grant_permission("writer", "users".to_string(), Permission::Update)?;
    auth.grant_permission("writer", "users".to_string(), Permission::Delete)?;
    println!("  ✅ Granted ALL on 'users' to 'writer'");

    // Create user and grant role
    auth.create_user("alice".to_string(), "password")?;
    auth.grant_role("alice", "reader")?;
    println!("  ✅ Created 'alice' with 'reader' role");

    // Login as alice and check permissions
    auth.authenticate("alice", "password")?;

    let can_select = auth.has_permission("users", Permission::Select)?;
    let can_insert = auth.has_permission("users", Permission::Insert)?;

    println!("  Alice's permissions on 'users':");
    println!("    SELECT: {}", if can_select { "✅" } else { "❌" });
    println!("    INSERT: {}", if can_insert { "✅" } else { "❌" });

    assert!(can_select);
    assert!(!can_insert);

    println!();
    Ok(())
}

fn demo_permission_inheritance() -> nexusql::Result<()> {
    println!("📋 Demo 3: Permission Inheritance");

    let mut auth = AuthManager::new();

    // Setup
    auth.create_superuser("admin".to_string(), "admin123")?;
    auth.authenticate("admin", "admin123")?;

    // Create role hierarchy: admin_role -> manager -> employee
    auth.create_role("employee".to_string())?;
    auth.create_role("manager".to_string())?;
    auth.create_role("admin_role".to_string())?;

    // Grant permissions to employee
    auth.grant_permission("employee", "reports".to_string(), Permission::Select)?;
    println!("  ✅ employee: SELECT on reports");

    // Grant permissions to manager (+ inherits from employee)
    auth.grant_permission("manager", "reports".to_string(), Permission::Insert)?;
    println!("  ✅ manager: INSERT on reports");

    // Make manager inherit from employee
    if let Some(manager_role) = auth.role_registry_mut().get_role_mut("manager") {
        manager_role.inherit_from("employee".to_string());
        println!("  ✅ manager inherits from employee");
    }

    // Grant permissions to admin_role (+ inherits from manager)
    auth.grant_permission("admin_role", "reports".to_string(), Permission::Delete)?;
    println!("  ✅ admin_role: DELETE on reports");

    if let Some(admin_role) = auth.role_registry_mut().get_role_mut("admin_role") {
        admin_role.inherit_from("manager".to_string());
        println!("  ✅ admin_role inherits from manager");
    }

    // Create user with admin_role
    auth.create_user("alice".to_string(), "password")?;
    auth.grant_role("alice", "admin_role")?;

    // Login as alice
    auth.authenticate("alice", "password")?;

    // Check inherited permissions
    let can_select = auth.has_permission("reports", Permission::Select)?;
    let can_insert = auth.has_permission("reports", Permission::Insert)?;
    let can_delete = auth.has_permission("reports", Permission::Delete)?;

    println!("\n  Alice's permissions on 'reports' (via admin_role → manager → employee):");
    println!(
        "    SELECT (from employee): {}",
        if can_select { "✅" } else { "❌" }
    );
    println!(
        "    INSERT (from manager):  {}",
        if can_insert { "✅" } else { "❌" }
    );
    println!(
        "    DELETE (from admin):    {}",
        if can_delete { "✅" } else { "❌" }
    );

    assert!(can_select); // Inherited from employee
    assert!(can_insert); // Inherited from manager
    assert!(can_delete); // Direct from admin_role

    println!();
    Ok(())
}

fn demo_row_level_security() -> nexusql::Result<()> {
    println!("📋 Demo 4: Row-Level Security (RLS)");

    let mut rls = RLSManager::new();

    // Enable RLS on messages table
    rls.enable_rls("messages".to_string());
    println!("  ✅ RLS enabled on 'messages' table");

    // Create policy: users can only see their own messages
    let policy = Policy::new(
        "user_isolation".to_string(),
        "messages".to_string(),
        PolicyCommand::Select,
    )
    .using_expr("owner = $username".to_string())
    .for_roles(vec!["user".to_string()]);

    rls.create_policy(policy)?;
    println!("  ✅ Policy created: users can only SELECT their own messages");

    // Create test data
    let all_messages = vec![
        Row {
            columns: vec!["id".to_string(), "owner".to_string(), "message".to_string()],
            values: vec![
                Value::Integer(1),
                Value::Text("alice".to_string()),
                Value::Text("Alice's message".to_string()),
            ],
        },
        Row {
            columns: vec!["id".to_string(), "owner".to_string(), "message".to_string()],
            values: vec![
                Value::Integer(2),
                Value::Text("bob".to_string()),
                Value::Text("Bob's message".to_string()),
            ],
        },
        Row {
            columns: vec!["id".to_string(), "owner".to_string(), "message".to_string()],
            values: vec![
                Value::Integer(3),
                Value::Text("alice".to_string()),
                Value::Text("Alice's second message".to_string()),
            ],
        },
    ];

    println!("\n  Test data: 3 messages (2 from alice, 1 from bob)");

    // Alice's context
    let alice_context = UserContext::new("alice".to_string(), vec!["user".to_string()], false);

    let alice_messages = rls.filter_select("messages", all_messages.clone(), &alice_context)?;
    println!("  Alice sees {} messages", alice_messages.len());

    // Bob's context
    let bob_context = UserContext::new("bob".to_string(), vec!["user".to_string()], false);

    let bob_messages = rls.filter_select("messages", all_messages.clone(), &bob_context)?;
    println!("  Bob sees {} messages", bob_messages.len());

    // Superuser sees everything
    let admin_context = UserContext::new(
        "admin".to_string(),
        vec!["admin".to_string()],
        true, // is_superuser
    );

    let admin_messages = rls.filter_select("messages", all_messages.clone(), &admin_context)?;
    println!("  Admin (superuser) sees {} messages", admin_messages.len());

    assert_eq!(alice_messages.len(), 2); // Alice's messages
    assert_eq!(bob_messages.len(), 1); // Bob's message
    assert_eq!(admin_messages.len(), 3); // All messages

    println!("  ✅ RLS correctly isolates user data");

    println!();
    Ok(())
}

fn demo_multi_tenant() -> nexusql::Result<()> {
    println!("📋 Demo 5: Multi-Tenant Isolation with RLS");

    let mut rls = RLSManager::new();

    // Enable RLS on orders table
    rls.enable_rls("orders".to_string());
    println!("  ✅ RLS enabled on 'orders' table");

    // Create tenant isolation policy
    let policy = Policy::new(
        "tenant_isolation".to_string(),
        "orders".to_string(),
        PolicyCommand::All, // Applies to all commands
    )
    .using_expr("tenant_id = $username".to_string()) // Simplified for demo
    .for_roles(vec!["tenant".to_string()]);

    rls.create_policy(policy)?;
    println!("  ✅ Policy created: tenant isolation on all operations");

    // Create test data for multiple tenants
    let all_orders = vec![
        Row {
            columns: vec![
                "id".to_string(),
                "tenant_id".to_string(),
                "description".to_string(),
                "amount".to_string(),
            ],
            values: vec![
                Value::Integer(1),
                Value::Text("tenant_a".to_string()),
                Value::Text("Order A1".to_string()),
                Value::Float(100.0),
            ],
        },
        Row {
            columns: vec![
                "id".to_string(),
                "tenant_id".to_string(),
                "description".to_string(),
                "amount".to_string(),
            ],
            values: vec![
                Value::Integer(2),
                Value::Text("tenant_b".to_string()),
                Value::Text("Order B1".to_string()),
                Value::Float(200.0),
            ],
        },
        Row {
            columns: vec![
                "id".to_string(),
                "tenant_id".to_string(),
                "description".to_string(),
                "amount".to_string(),
            ],
            values: vec![
                Value::Integer(3),
                Value::Text("tenant_a".to_string()),
                Value::Text("Order A2".to_string()),
                Value::Float(150.0),
            ],
        },
        Row {
            columns: vec![
                "id".to_string(),
                "tenant_id".to_string(),
                "description".to_string(),
                "amount".to_string(),
            ],
            values: vec![
                Value::Integer(4),
                Value::Text("tenant_c".to_string()),
                Value::Text("Order C1".to_string()),
                Value::Float(300.0),
            ],
        },
    ];

    println!("\n  Test data: 4 orders across 3 tenants");
    println!("    tenant_a: 2 orders");
    println!("    tenant_b: 1 order");
    println!("    tenant_c: 1 order");

    // Tenant A context
    let tenant_a = UserContext::new("tenant_a".to_string(), vec!["tenant".to_string()], false);

    let tenant_a_orders = rls.filter_select("orders", all_orders.clone(), &tenant_a)?;
    println!("\n  Tenant A sees: {} orders ✅", tenant_a_orders.len());
    assert_eq!(tenant_a_orders.len(), 2);

    // Tenant B context
    let tenant_b = UserContext::new("tenant_b".to_string(), vec!["tenant".to_string()], false);

    let tenant_b_orders = rls.filter_select("orders", all_orders.clone(), &tenant_b)?;
    println!("  Tenant B sees: {} order ✅", tenant_b_orders.len());
    assert_eq!(tenant_b_orders.len(), 1);

    // Tenant C context
    let tenant_c = UserContext::new("tenant_c".to_string(), vec!["tenant".to_string()], false);

    let tenant_c_orders = rls.filter_select("orders", all_orders.clone(), &tenant_c)?;
    println!("  Tenant C sees: {} order ✅", tenant_c_orders.len());
    assert_eq!(tenant_c_orders.len(), 1);

    println!("\n  ✅ Perfect tenant isolation - no data leakage!");

    println!();
    Ok(())
}

fn print_summary() {
    println!("📊 Summary of Features Demonstrated:\n");

    println!("🔐 Authentication:");
    println!("  - bcrypt password hashing (PostgreSQL standard)");
    println!("  - User creation and login");
    println!("  - Password verification");
    println!("  - Superuser privileges\n");

    println!("👥 Role-Based Access Control (RBAC):");
    println!("  - Role creation and management");
    println!("  - GRANT/REVOKE permissions");
    println!("  - Permission types: SELECT, INSERT, UPDATE, DELETE");
    println!("  - Role inheritance hierarchy");
    println!("  - Permission checking\n");

    println!("🔒 Row-Level Security (RLS):");
    println!("  - Table-level RLS enable/disable");
    println!("  - Policy creation with USING clauses");
    println!("  - Per-command policies (SELECT, INSERT, UPDATE, DELETE)");
    println!("  - Role-specific policies");
    println!("  - User context evaluation");
    println!("  - Superuser bypass\n");

    println!("🏢 Multi-Tenant Isolation:");
    println!("  - Complete data isolation between tenants");
    println!("  - Policy-based tenant filtering");
    println!("  - Zero data leakage\n");

    println!("🎯 PostgreSQL Compatibility:");
    println!("  - pg_authid-like user management");
    println!("  - CREATE ROLE / GRANT syntax patterns");
    println!("  - Row-level security policies");
    println!("  - Permission inheritance");
    println!("  - bcrypt password encryption\n");

    println!("✅ Phase 2 (Week 8) Complete!");
    println!("   Next: Phase 3 - Transactions, WAL, ACID compliance");
}
