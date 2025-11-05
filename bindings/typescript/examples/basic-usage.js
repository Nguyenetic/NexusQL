/**
 * Basic Usage Example
 *
 * Demonstrates fundamental NexusQL operations:
 * - Creating a database
 * - Creating tables
 * - Inserting data with parameterized queries
 * - Querying data
 * - Closing the database
 */

const { NexusDb } = require('../index.js');

async function main() {
  console.log('🚀 NexusQL Basic Usage Example\n');

  // Use in-memory database for this example
  const db = new NexusDb(':memory:');

  try {
    // 1. Create table
    console.log('1️⃣  Creating users table...');
    await db.execute(`
      CREATE TABLE users (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        email TEXT UNIQUE,
        age INTEGER,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP
      )
    `);
    console.log('✅ Table created\n');

    // 2. Insert data with parameterized queries (prevents SQL injection)
    console.log('2️⃣  Inserting users...');
    await db.execute(
      'INSERT INTO users (id, name, email, age) VALUES ($1, $2, $3, $4)',
      [1, 'Alice', 'alice@example.com', 30]
    );
    await db.execute(
      'INSERT INTO users (id, name, email, age) VALUES ($1, $2, $3, $4)',
      [2, 'Bob', 'bob@example.com', 25]
    );
    await db.execute(
      'INSERT INTO users (id, name, email, age) VALUES ($1, $2, $3, $4)',
      [3, 'Charlie', 'charlie@example.com', 35]
    );
    console.log('✅ 3 users inserted\n');

    // 3. Query all users
    console.log('3️⃣  Querying all users...');
    const allUsers = await db.query('SELECT * FROM users ORDER BY id');
    console.log(`Found ${allUsers.rowCount} users:`);
    console.table(allUsers.rows);

    // 4. Query with WHERE clause and parameters
    console.log('4️⃣  Querying users older than 28...');
    const olderUsers = await db.query(
      'SELECT name, email, age FROM users WHERE age > $1 ORDER BY age',
      [28]
    );
    console.log(`Found ${olderUsers.rowCount} users:`);
    console.table(olderUsers.rows);

    // 5. Update data
    console.log('\n5️⃣  Updating user...');
    const updated = await db.execute(
      'UPDATE users SET email = $1 WHERE id = $2',
      ['alice.smith@example.com', 1]
    );
    console.log(`✅ Updated ${updated} row(s)\n`);

    // 6. Verify update
    const updatedUser = await db.query('SELECT * FROM users WHERE id = $1', [1]);
    console.log('Updated user:');
    console.table(updatedUser.rows);

    // 7. Delete data
    console.log('7️⃣  Deleting user...');
    const deleted = await db.execute('DELETE FROM users WHERE id = $1', [3]);
    console.log(`✅ Deleted ${deleted} row(s)\n`);

    // 8. Final count
    const finalCount = await db.query('SELECT COUNT(*) as total FROM users');
    console.log(`📊 Final user count: ${finalCount.rows[0].total}`);

  } catch (error) {
    console.error('❌ Error:', error.message);
  } finally {
    // Always close the database
    await db.close();
    console.log('\n✅ Database closed');
  }
}

main().catch(console.error);
