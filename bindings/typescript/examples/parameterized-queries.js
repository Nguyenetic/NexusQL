/**
 * Parameterized Queries Example
 *
 * Demonstrates SQL injection prevention using parameterized queries
 */

const { NexusDb } = require('../index.js');

async function main() {
  console.log('🔒 Parameterized Queries & SQL Injection Prevention\n');

  const db = new NexusDb(':memory:');

  try {
    // Setup
    await db.execute(`
      CREATE TABLE users (
        id INTEGER PRIMARY KEY,
        username TEXT UNIQUE NOT NULL,
        email TEXT
      )
    `);

    console.log('1️⃣  Inserting safe data with parameterized queries...\n');

    // ✅ SAFE - Parameterized query
    await db.execute(
      'INSERT INTO users VALUES ($1, $2, $3)',
      [1, 'alice', 'alice@example.com']
    );
    await db.execute(
      'INSERT INTO users VALUES ($1, $2, $3)',
      [2, 'bob', 'bob@example.com']
    );
    console.log('✅ Inserted 2 users safely\n');

    // 2. Demonstrate SQL injection prevention
    console.log('2️⃣  Testing SQL injection prevention...\n');

    // Malicious inputs that would cause SQL injection if not properly handled
    const maliciousInputs = [
      "'; DROP TABLE users; --",
      "' OR '1'='1",
      "admin'--",
      "1' UNION SELECT * FROM users--"
    ];

    for (const maliciousInput of maliciousInputs) {
      try {
        // ✅ SAFE - Parameterized query treats input as literal data
        await db.execute(
          'INSERT INTO users VALUES ($1, $2, $3)',
          [maliciousInputs.indexOf(maliciousInput) + 10, maliciousInput, 'test@example.com']
        );
        console.log(`✅ Safely stored malicious input: "${maliciousInput}"`);
      } catch (error) {
        console.log(`⚠️  Constraint violation (expected): ${error.message}`);
      }
    }

    // 3. Verify table still exists (SQL injection was prevented)
    console.log('\n3️⃣  Verifying database integrity...');
    const allUsers = await db.query('SELECT * FROM users');
    console.log(`✅ Table intact with ${allUsers.rowCount} rows`);
    console.log('✅ SQL injection attacks were prevented!\n');

    // 4. Safe query with user input
    console.log('4️⃣  Querying with user input safely...');
    const userId = '1'; // Simulated user input

    // ✅ SAFE - Parameterized query
    const result = await db.query(
      'SELECT * FROM users WHERE id = $1',
      [userId]
    );
    console.log(`Found user:`, result.rows[0]);

    // 5. Multiple parameters
    console.log('\n5️⃣  Using multiple parameters...');
    const searchTerm = 'alice';
    const minId = 1;

    const searchResult = await db.query(
      'SELECT * FROM users WHERE username LIKE $1 AND id >= $2',
      [`%${searchTerm}%`, minId]
    );
    console.log(`Search results:`, searchResult.rows);

    console.log('\n✅ All operations completed safely!');

  } catch (error) {
    console.error('❌ Error:', error.message);
  } finally {
    await db.close();
    console.log('✅ Database closed');
  }
}

// Educational note
console.log(`
⚠️  SECURITY WARNING:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

❌ NEVER use string concatenation with user input:
   await db.execute(\`INSERT INTO users VALUES ('\${userId}', '\${username}')\`);

✅ ALWAYS use parameterized queries:
   await db.execute('INSERT INTO users VALUES ($1, $2)', [userId, username]);

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
`);

main().catch(console.error);
