# Security Policy

## Supported Versions

The following versions of NexusQL are currently supported with security updates:

| Version | Supported          | Notes |
| ------- | ------------------ | ----- |
| 3.0.x   | :white_check_mark: | Current stable release with parameterized queries |
| 2.7.x   | :white_check_mark: | Supported with `prepare()` API |
| 2.6.x   | :x:                | Upgrade to 3.0.x for security fixes |
| < 2.6   | :x:                | Not supported |

## Security Features

### Parameterized Queries (SQL Injection Prevention)

**Status:** ✅ **Fully Implemented** (v3.0.0+)

NexusQL provides comprehensive SQL injection prevention through parameterized queries:

```typescript
// ✅ SAFE - Parameterized query
await db.execute('INSERT INTO users VALUES ($1, $2)', [userId, userName]);

// ❌ UNSAFE - String concatenation (DO NOT USE)
await db.execute(`INSERT INTO users VALUES (${userId}, '${userName}')`);
```

**Best Practices:**
- Always use parameterized queries with `$1, $2, ...` placeholders
- Never concatenate user input into SQL strings
- Use `prepare()` for repeated queries with different parameters

### Prepared Statements

**Status:** ✅ **Fully Implemented** (v2.7.0+)

Prepared statements provide both performance and security benefits:

```typescript
const stmt = await db.prepare('SELECT * FROM users WHERE id = $1');
const result = await stmt.execute(userId);
await stmt.close();
```

### Transaction Security

**Status:** ⚠️ **Limited** (Parameterized queries not yet supported in transactions)

Current limitations:
- Transactions don't support parameterized queries yet
- Workaround: Use prepared statements outside transactions
- **Do not** use string concatenation with user input in transactions

**Secure transaction pattern:**
```typescript
// Use escaped strings as temporary workaround
const escapedName = userName.replace(/'/g, "''");
const tx = await db.beginTransaction();
await tx.query(`INSERT INTO users VALUES (1, '${escapedName}')`);
await tx.commit();
```

### Authentication & Authorization

**Status:** ✅ **Implemented** (RBAC, Row-Level Security)

- bcrypt password hashing
- Role-Based Access Control (RBAC)
- Row-Level Security (RLS) policies

**Recommendations:**
- Hash passwords with bcrypt (10+ rounds)
- Implement principle of least privilege
- Use RLS for multi-tenant applications

## Known Security Limitations

### 1. Transaction Parameterization (Planned for v3.1.0)

**Issue:** Parameterized queries not supported in `tx.query()`

**Workaround:**
```typescript
// Use prepared statements outside transactions
const stmt = await db.prepare('INSERT INTO users VALUES ($1, $2)');
await stmt.execute(id, name);
await stmt.close();
```

**Timeline:** Fix planned for v3.1.0 (Q2 2024)

### 2. File System Permissions

**Issue:** Database files created with default permissions

**Recommendation:**
```bash
# Set restrictive permissions on database files
chmod 600 ./database.db
```

### 3. In-Memory Database Security

**Issue:** In-memory databases (`:memory:`) are not encrypted

**Recommendation:**
- Use file-based databases for sensitive data
- Consider full-disk encryption for production systems

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue, please follow these steps:

### 1. Do Not Publicly Disclose

**DO NOT** create a public GitHub issue for security vulnerabilities.

### 2. Report Privately

Send a detailed report to: **[john@nexusql.io](mailto:john@nexusql.io)**

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if available)
- Your contact information

### 3. Response Timeline

- **24 hours:** Initial acknowledgment of your report
- **72 hours:** Preliminary assessment and severity classification
- **7 days:** Detailed response with timeline for fix
- **30 days:** Security patch released (for high-severity issues)

### 4. Coordinated Disclosure

- We will work with you to understand and fix the issue
- We will credit you in the security advisory (unless you prefer to remain anonymous)
- We request 90 days before public disclosure to allow users to update

### 5. Security Advisory Process

1. We confirm the vulnerability
2. We develop and test a fix
3. We prepare a security advisory
4. We release a patched version
5. We publish the advisory with CVE (if applicable)
6. We credit the reporter (unless anonymous)

## Security Best Practices

### For Application Developers

#### 1. Always Use Parameterized Queries

```typescript
// ✅ CORRECT
const result = await db.query(
  'SELECT * FROM users WHERE email = $1',
  [userEmail]
);

// ❌ WRONG - SQL injection vulnerability
const result = await db.query(
  `SELECT * FROM users WHERE email = '${userEmail}'`
);
```

#### 2. Validate Input

```typescript
function validateEmail(email: string): boolean {
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  return emailRegex.test(email);
}

if (!validateEmail(userEmail)) {
  throw new Error('Invalid email format');
}

await db.execute('INSERT INTO users (email) VALUES ($1)', [userEmail]);
```

#### 3. Use Prepared Statements for Repeated Queries

```typescript
const stmt = await db.prepare('SELECT * FROM users WHERE id = $1');

// Safe and efficient
for (const userId of userIds) {
  const result = await stmt.execute(userId);
  // Process result
}

await stmt.close();
```

#### 4. Implement Least Privilege

```typescript
// Create read-only user for queries
await db.execute('CREATE USER readonly WITH PASSWORD $1', [hashedPassword]);
await db.execute('GRANT SELECT ON ALL TABLES TO readonly');
```

#### 5. Encrypt Sensitive Data

```typescript
import crypto from 'crypto';

function encrypt(text: string, key: string): string {
  const cipher = crypto.createCipher('aes-256-cbc', key);
  return cipher.update(text, 'utf8', 'hex') + cipher.final('hex');
}

const encryptedSSN = encrypt(ssn, process.env.ENCRYPTION_KEY!);
await db.execute('INSERT INTO users (ssn_encrypted) VALUES ($1)', [encryptedSSN]);
```

#### 6. Use Secure Database Paths

```typescript
// ✅ GOOD - Secure location
const db = new NexusDb('/var/lib/myapp/database.db');

// ❌ BAD - World-readable location
const db = new NexusDb('/tmp/database.db');
```

#### 7. Handle Errors Securely

```typescript
try {
  await db.execute('INSERT INTO users VALUES ($1, $2)', [id, email]);
} catch (error) {
  // ❌ DON'T expose sensitive error details to users
  console.error('Full error:', error);

  // ✅ DO log errors securely and return generic message
  logger.error('Database error:', error);
  throw new Error('An error occurred. Please try again later.');
}
```

### For System Administrators

#### 1. File Permissions

```bash
# Database file
chmod 600 /path/to/database.db
chown app:app /path/to/database.db

# Database directory
chmod 700 /path/to/data
chown app:app /path/to/data
```

#### 2. Network Security

- Run database applications behind firewalls
- Use HTTPS for all client-server communication
- Implement rate limiting to prevent DoS

#### 3. Regular Backups

```bash
# Automated backup script
#!/bin/bash
BACKUP_DIR="/var/backups/nexusql"
DB_FILE="/var/lib/myapp/database.db"

mkdir -p "$BACKUP_DIR"
cp "$DB_FILE" "$BACKUP_DIR/backup-$(date +%Y%m%d-%H%M%S).db"

# Keep only last 7 days
find "$BACKUP_DIR" -name "backup-*.db" -mtime +7 -delete
```

#### 4. Monitor Access

```typescript
// Log all database operations
const originalExecute = db.execute.bind(db);
db.execute = async (sql: string, params?: any[]) => {
  logger.info('Database operation:', { sql, params, user: currentUser });
  return originalExecute(sql, params);
};
```

#### 5. Update Regularly

```bash
# Check for updates
npm outdated @nguyenetic/nexusql

# Update to latest secure version
npm update @nguyenetic/nexusql
```

## Security Checklist

Use this checklist when deploying NexusQL applications:

- [ ] All queries use parameterized placeholders (`$1, $2, ...`)
- [ ] No string concatenation with user input
- [ ] Input validation implemented
- [ ] Prepared statements used for repeated queries
- [ ] Database files have restrictive permissions (600)
- [ ] Sensitive data is encrypted before storage
- [ ] Error messages don't expose sensitive information
- [ ] Regular backups are automated
- [ ] Application runs with least privilege
- [ ] Security updates are applied promptly
- [ ] Access logging is enabled
- [ ] Rate limiting is configured
- [ ] HTTPS is used for all connections

## Security Resources

- **Documentation:** [Security Considerations](docs/guides/security.md)
- **Examples:** [Secure Coding Patterns](bindings/typescript/examples/)
- **Testing:** [Security Test Suite](tests/security/)
- **OWASP Top 10:** https://owasp.org/www-project-top-ten/

## Acknowledgments

We thank the security researchers who have helped improve NexusQL:

<!-- List will be populated as researchers report vulnerabilities -->

---

**Last Updated:** 2024-11-04
**Contact:** [john@nexusql.io](mailto:john@nexusql.io)
