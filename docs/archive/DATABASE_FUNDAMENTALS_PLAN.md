# NexusQL Database Fundamentals Implementation Plan

## Overview

Based on Context7 research of PostgreSQL, this document outlines the critical database fundamentals that NexusQL needs to become a production-ready database.

---

## Current State Assessment

### ✅ What We Have
- Fast queries (1.4-2M inserts/sec, 741K qps with indexes)
- B-Tree indexes
- Query caching
- Materialized views
- Parallel execution
- Adaptive query execution

### ❌ What We're Missing (Critical Gaps)
1. **No Schema System** - Tables have no enforced data types
2. **No Constraints** - No PRIMARY KEY, FOREIGN KEY, CHECK, UNIQUE, NOT NULL
3. **No User Authentication** - No user management
4. **No Authorization** - No role-based access control (RBAC)
5. **No Row-Level Security** - No fine-grained access control
6. **No Data Validation** - No input sanitization
7. **No SQL Injection Protection** - Vulnerable to attacks
8. **No Transactions** - No ACID guarantees
9. **No Audit Logging** - No security audit trail

---

## Phase 1: Schema System & Constraints (Week 7)

### 1.1 Schema Definition

**Based on PostgreSQL:** `CREATE SCHEMA`, `information_schema`

**Implementation:**
```rust
pub struct Schema {
    pub name: String,
    pub owner: String,
    pub tables: HashMap<String, TableSchema>,
    pub created_at: u64,
}

pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub constraints: Vec<Constraint>,
    pub indexes: Vec<IndexDef>,
}

pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub default_value: Option<Value>,
    pub constraints: Vec<ColumnConstraint>,
}

pub enum DataType {
    Integer,
    BigInt,
    SmallInt,
    Float,
    Double,
    Text,
    Varchar(usize),
    Boolean,
    Date,
    Timestamp,
    Json,
    Blob,
}
```

**Features:**
- ✅ Multiple schemas per database
- ✅ Schema ownership
- ✅ Fully typed columns
- ✅ Default values
- ✅ NULL/NOT NULL enforcement

---

### 1.2 Constraint System

**Based on PostgreSQL:** `PRIMARY KEY`, `FOREIGN KEY`, `UNIQUE`, `CHECK`, `NOT NULL`

**Implementation:**
```rust
pub enum Constraint {
    PrimaryKey {
        name: String,
        columns: Vec<String>,
    },
    ForeignKey {
        name: String,
        columns: Vec<String>,
        ref_table: String,
        ref_columns: Vec<String>,
        on_delete: ReferentialAction,
        on_update: ReferentialAction,
    },
    Unique {
        name: String,
        columns: Vec<String>,
        nulls_distinct: bool,
    },
    Check {
        name: String,
        expression: String,
        no_inherit: bool,
    },
}

pub enum ReferentialAction {
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

pub enum ColumnConstraint {
    NotNull,
    Null,
    Unique,
    PrimaryKey,
    Check(String),
    Default(Value),
    References {
        table: String,
        column: String,
        on_delete: ReferentialAction,
        on_update: ReferentialAction,
    },
}
```

**Features:**
- ✅ PRIMARY KEY (single & composite)
- ✅ FOREIGN KEY with ON DELETE/ON UPDATE
- ✅ UNIQUE (with NULLS DISTINCT/NOT DISTINCT)
- ✅ CHECK constraints
- ✅ NOT NULL enforcement
- ✅ Referential integrity validation

---

### 1.3 Data Validation

**Implementation:**
```rust
pub struct Validator {
    schema: Arc<TableSchema>,
}

impl Validator {
    pub fn validate_insert(&self, row: &Row) -> Result<()> {
        // 1. Check column count
        // 2. Check data types
        // 3. Check NOT NULL constraints
        // 4. Check UNIQUE constraints
        // 5. Check PRIMARY KEY uniqueness
        // 6. Check FOREIGN KEY references
        // 7. Check CHECK constraints
        // 8. Apply DEFAULT values
    }

    pub fn validate_update(&self, old: &Row, new: &Row) -> Result<()> {
        // Similar to insert but also check:
        // - PRIMARY KEY immutability
        // - FOREIGN KEY cascades
    }

    pub fn validate_delete(&self, row: &Row) -> Result<()> {
        // Check if row is referenced by foreign keys
        // Apply ON DELETE actions
    }
}
```

---

## Phase 2: Authentication & Authorization (Week 8)

### 2.1 User Management

**Based on PostgreSQL:** `CREATE ROLE`, `CREATE USER`, `ALTER ROLE`, `DROP ROLE`

**Implementation:**
```rust
pub struct User {
    pub username: String,
    pub password_hash: String, // bcrypt hash
    pub created_at: u64,
    pub valid_until: Option<u64>,
    pub is_superuser: bool,
    pub can_create_db: bool,
    pub can_create_role: bool,
    pub connection_limit: i32,
}

pub struct AuthenticationManager {
    users: HashMap<String, User>,
    sessions: HashMap<String, Session>,
}

impl AuthenticationManager {
    pub fn authenticate(&mut self, username: &str, password: &str) -> Result<String> {
        // 1. Find user
        // 2. Verify bcrypt hash
        // 3. Check valid_until
        // 4. Check connection_limit
        // 5. Create session token (JWT or UUID)
        // 6. Return session token
    }

    pub fn create_user(&mut self, username: &str, password: &str, is_superuser: bool) -> Result<()> {
        // 1. Validate username (alphanumeric, not reserved)
        // 2. Check if user exists
        // 3. Hash password with bcrypt
        // 4. Store user
    }
}
```

**Features:**
- ✅ Password hashing with bcrypt (work factor 12)
- ✅ Session management
- ✅ Connection limits
- ✅ Account expiration
- ✅ Superuser privileges

---

### 2.2 Role-Based Access Control (RBAC)

**Based on PostgreSQL:** `CREATE ROLE`, `GRANT`, `REVOKE`, Role inheritance

**Implementation:**
```rust
pub struct Role {
    pub name: String,
    pub members: Vec<String>, // user names
    pub member_of: Vec<String>, // parent roles
    pub privileges: HashMap<String, TablePrivileges>,
}

pub struct TablePrivileges {
    pub select: bool,
    pub insert: bool,
    pub update: bool,
    pub delete: bool,
    pub references: bool,
    pub truncate: bool,
    pub column_privileges: HashMap<String, ColumnPrivileges>,
}

pub struct ColumnPrivileges {
    pub select: bool,
    pub insert: bool,
    pub update: bool,
}

impl RBACManager {
    pub fn grant(&mut self, role: &str, privilege: Privilege, object: &str) -> Result<()> {
        // GRANT SELECT, INSERT, UPDATE, DELETE ON table TO role
    }

    pub fn revoke(&mut self, role: &str, privilege: Privilege, object: &str) -> Result<()> {
        // REVOKE SELECT ON table FROM role
    }

    pub fn check_permission(&self, user: &str, table: &str, action: Action) -> Result<bool> {
        // 1. Get user's roles (including inherited)
        // 2. Collect all privileges from roles
        // 3. Check if action is permitted
    }
}
```

**Features:**
- ✅ Role hierarchy (role inheritance)
- ✅ Table-level permissions
- ✅ Column-level permissions
- ✅ GRANT/REVOKE operations
- ✅ Permission checking on every query

---

### 2.3 Row-Level Security (RLS)

**Based on PostgreSQL:** `CREATE POLICY`, `ALTER TABLE ENABLE ROW LEVEL SECURITY`

**Implementation:**
```rust
pub struct Policy {
    pub name: String,
    pub table: String,
    pub command: PolicyCommand,
    pub roles: Vec<String>, // Empty means all roles
    pub using_clause: String, // SQL expression
    pub with_check_clause: Option<String>, // SQL expression
}

pub enum PolicyCommand {
    All,
    Select,
    Insert,
    Update,
    Delete,
}

impl RLSManager {
    pub fn create_policy(&mut self, policy: Policy) -> Result<()> {
        // Store policy
    }

    pub fn filter_rows(&self, table: &str, rows: Vec<Row>, user: &str, action: Action) -> Result<Vec<Row>> {
        // 1. Get policies for table and action
        // 2. Evaluate USING clause for each row
        // 3. Return only rows that pass policy
    }

    pub fn check_insert(&self, table: &str, row: &Row, user: &str) -> Result<()> {
        // Evaluate WITH CHECK clause
    }
}
```

**Example Policies (PostgreSQL style):**
```sql
-- Admin can see everything
CREATE POLICY admin_all ON passwd TO admin USING (true) WITH CHECK (true);

-- Users can only see their own row
CREATE POLICY user_self ON passwd FOR SELECT USING (username = current_user);

-- Users can update their own row but with restrictions
CREATE POLICY user_update ON passwd FOR UPDATE
  USING (username = current_user)
  WITH CHECK (
    username = current_user AND
    shell IN ('/bin/bash', '/bin/sh', '/bin/zsh')
  );
```

**Features:**
- ✅ Per-table policies
- ✅ Per-command policies (SELECT, INSERT, UPDATE, DELETE)
- ✅ Per-role policies
- ✅ USING clause for SELECT/UPDATE/DELETE
- ✅ WITH CHECK clause for INSERT/UPDATE
- ✅ Policy evaluation on every query

---

## Phase 3: Security Features (Week 8 continued)

### 3.1 SQL Injection Protection

**Implementation:**
```rust
pub struct QuerySanitizer {
    // Prepared statements with parameter binding
}

impl QuerySanitizer {
    pub fn prepare(&self, sql: &str) -> Result<PreparedStatement> {
        // 1. Parse SQL
        // 2. Identify parameter placeholders ($1, $2, etc.)
        // 3. Store prepared plan
        // 4. Return statement ID
    }

    pub fn execute(&self, stmt_id: &str, params: Vec<Value>) -> Result<ResultSet> {
        // 1. Get prepared plan
        // 2. Bind parameters (escape automatically)
        // 3. Execute
        // 4. Return results
    }
}
```

**Features:**
- ✅ Prepared statements
- ✅ Parameter binding
- ✅ Automatic escaping
- ✅ No string concatenation in queries

---

### 3.2 Audit Logging

**Based on PostgreSQL:** `pg_audit` extension

**Implementation:**
```rust
pub struct AuditLog {
    entries: Vec<AuditEntry>,
}

pub struct AuditEntry {
    pub timestamp: u64,
    pub user: String,
    pub action: String, // SELECT, INSERT, UPDATE, DELETE, CREATE, etc.
    pub object_type: String, // TABLE, INDEX, ROLE, etc.
    pub object_name: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub ip_address: Option<String>,
    pub query: String,
}

impl AuditLog {
    pub fn log_query(&mut self, user: &str, query: &str, success: bool) {
        // Log every query
    }

    pub fn log_ddl(&mut self, user: &str, ddl: &str, object: &str) {
        // Log schema changes
    }

    pub fn log_auth(&mut self, user: &str, success: bool, reason: Option<String>) {
        // Log authentication attempts
    }
}
```

**Features:**
- ✅ Log all queries
- ✅ Log authentication attempts
- ✅ Log schema changes
- ✅ Log permission denials
- ✅ Queryable audit log

---

### 3.3 Connection Security

**Implementation:**
```rust
pub struct ConnectionManager {
    pub max_connections: usize,
    pub idle_timeout: u64,
    pub require_ssl: bool,
}

impl ConnectionManager {
    pub fn enforce_limits(&self) -> Result<()> {
        // 1. Check connection count
        // 2. Check idle timeout
        // 3. Close stale connections
    }
}
```

**Features:**
- ✅ Connection limits
- ✅ Idle timeouts
- ✅ SSL/TLS support (future)

---

## Phase 4: Transactions (Week 9)

### 4.1 ACID Transactions

**Based on PostgreSQL:** `BEGIN`, `COMMIT`, `ROLLBACK`, `SAVEPOINT`

**Implementation:**
```rust
pub struct Transaction {
    pub id: u64,
    pub isolation_level: IsolationLevel,
    pub read_only: bool,
    pub uncommitted_changes: Vec<Change>,
    pub savepoints: Vec<Savepoint>,
}

pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

pub struct Change {
    pub operation: Operation,
    pub table: String,
    pub row_id: u64,
    pub old_value: Option<Row>,
    pub new_value: Option<Row>,
}

pub enum Operation {
    Insert,
    Update,
    Delete,
}

impl TransactionManager {
    pub fn begin(&mut self) -> Result<u64> {
        // Create transaction
    }

    pub fn commit(&mut self, tx_id: u64) -> Result<()> {
        // 1. Validate all constraints
        // 2. Check for conflicts
        // 3. Apply changes
        // 4. Write to WAL
    }

    pub fn rollback(&mut self, tx_id: u64) -> Result<()> {
        // Discard uncommitted changes
    }

    pub fn savepoint(&mut self, tx_id: u64, name: &str) -> Result<()> {
        // Create savepoint
    }

    pub fn rollback_to_savepoint(&mut self, tx_id: u64, name: &str) -> Result<()> {
        // Rollback to savepoint
    }
}
```

**Features:**
- ✅ BEGIN/COMMIT/ROLLBACK
- ✅ SAVEPOINT support
- ✅ Isolation levels
- ✅ Conflict detection
- ✅ Read-only transactions

---

## Implementation Priority

### Week 7: Foundation
1. **Day 1-2:** Schema system (TableSchema, ColumnDef, DataType)
2. **Day 3-4:** Constraint system (PRIMARY KEY, FOREIGN KEY, UNIQUE, CHECK)
3. **Day 5-6:** Data validation (Validator)
4. **Day 7:** Testing and documentation

### Week 8: Security
1. **Day 1-2:** User authentication (password hashing, sessions)
2. **Day 3-4:** RBAC (roles, privileges, GRANT/REVOKE)
3. **Day 5-6:** Row-level security (policies)
4. **Day 7:** Audit logging and testing

### Week 9: Transactions
1. **Day 1-3:** Transaction management (BEGIN/COMMIT/ROLLBACK)
2. **Day 4-5:** Write-ahead logging (WAL)
3. **Day 6:** Isolation levels
4. **Day 7:** Testing and benchmarking

---

## Security Best Practices (from PostgreSQL)

### 1. Password Security
- ✅ Use bcrypt (work factor 12 minimum)
- ✅ Never store plaintext passwords
- ✅ Enforce password complexity
- ✅ Account lockout after failed attempts

### 2. Access Control
- ✅ Principle of least privilege
- ✅ Role-based access (not user-based)
- ✅ Separate read-only roles
- ✅ Row-level security for sensitive data

### 3. SQL Injection Prevention
- ✅ Always use prepared statements
- ✅ Never concatenate user input
- ✅ Parameterize all queries
- ✅ Validate and sanitize input

### 4. Audit & Monitoring
- ✅ Log all authentication attempts
- ✅ Log schema changes
- ✅ Log permission denials
- ✅ Regular security audits

---

## Example Usage

### Creating a Secure Table
```sql
-- Create schema
CREATE SCHEMA app AUTHORIZATION app_admin;

-- Create table with constraints
CREATE TABLE app.users (
    id INTEGER PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    is_active BOOLEAN DEFAULT TRUE,
    CHECK (LENGTH(username) >= 3),
    CHECK (email LIKE '%@%')
);

-- Create role
CREATE ROLE app_users;

-- Grant permissions
GRANT SELECT, INSERT ON app.users TO app_users;
GRANT UPDATE (email, is_active) ON app.users TO app_users;

-- Enable row-level security
ALTER TABLE app.users ENABLE ROW LEVEL SECURITY;

-- Users can only see their own row
CREATE POLICY user_isolation ON app.users
    FOR SELECT
    USING (username = current_user);

-- Users can only update their own email
CREATE POLICY user_update_self ON app.users
    FOR UPDATE
    USING (username = current_user)
    WITH CHECK (username = current_user);
```

---

## Testing Strategy

### Unit Tests
- Schema creation and validation
- Constraint enforcement
- Permission checking
- Policy evaluation

### Integration Tests
- End-to-end authentication
- Role hierarchy
- Foreign key cascades
- Transaction isolation

### Security Tests
- SQL injection attempts
- Privilege escalation attempts
- Unauthorized access attempts
- Audit log verification

---

## Success Metrics

**Schema & Constraints:**
- ✅ All PostgreSQL data types supported
- ✅ All constraint types implemented
- ✅ 100% constraint enforcement
- ✅ Zero invalid data accepted

**Security:**
- ✅ No SQL injection vulnerabilities
- ✅ 100% query authorization
- ✅ Complete audit trail
- ✅ Password security (bcrypt)

**Transactions:**
- ✅ ACID compliance
- ✅ Serializable isolation
- ✅ Crash recovery
- ✅ <5ms transaction overhead

---

## Documentation

Each feature will include:
1. Context7 research summary (PostgreSQL patterns)
2. Implementation details
3. Usage examples
4. Security considerations
5. Performance benchmarks

---

**Status:** Plan complete, ready for implementation

**Next Step:** Start with Schema System (Phase 1.1)
