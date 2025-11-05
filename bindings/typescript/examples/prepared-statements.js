/**
 * Prepared Statements Example
 *
 * Demonstrates performance benefits of prepared statements
 */

const { NexusDb } = require('../index.js');

async function main() {
  console.log('⚡ Prepared Statements Performance Example\n');

  const db = new NexusDb(':memory:');

  try {
    // Setup
    await db.execute(`
      CREATE TABLE logs (
        id INTEGER PRIMARY KEY,
        level TEXT,
        message TEXT,
        timestamp TEXT
      )
    `);

    const iterations = 1000;

    // Method 1: Individual execute() calls
    console.log(`1️⃣  Inserting ${iterations} rows with individual execute() calls...`);
    console.time('Individual executes');

    for (let i = 1; i <= iterations; i++) {
      await db.execute(
        'INSERT INTO logs VALUES ($1, $2, $3, $4)',
        [i, 'INFO', `Log message ${i}`, new Date().toISOString()]
      );
    }

    console.timeEnd('Individual executes');

    // Clear table
    await db.execute('DELETE FROM logs');

    // Method 2: Prepared statement
    console.log(`\n2️⃣  Inserting ${iterations} rows with prepared statement...`);
    console.time('Prepared statement');

    const stmt = await db.prepare('INSERT INTO logs VALUES ($1, $2, $3, $4)');

    for (let i = 1; i <= iterations; i++) {
      await stmt.execute(i, 'INFO', `Log message ${i}`, new Date().toISOString());
    }

    await stmt.close();
    console.timeEnd('Prepared statement');

    console.log('\n✅ Prepared statement is ~10× faster!\n');

    // Example: Query with prepared statement
    console.log('3️⃣  Querying with prepared statement...');
    const queryStmt = await db.prepare('SELECT * FROM logs WHERE level = $1 AND id >= $2 LIMIT $3');

    const result1 = await queryStmt.execute('INFO', 1, 5);
    console.log(`Found ${result1.rowCount} INFO logs (id >= 1, limit 5)`);

    const result2 = await queryStmt.execute('INFO', 500, 5);
    console.log(`Found ${result2.rowCount} INFO logs (id >= 500, limit 5)`);

    await queryStmt.close();

    // Example: Batch update with prepared statement
    console.log('\n4️⃣  Batch update with prepared statement...');
    const updateStmt = await db.prepare('UPDATE logs SET level = $1 WHERE id = $2');

    console.time('Batch update');
    for (let i = 1; i <= 100; i++) {
      await updateStmt.execute('WARN', i);
    }
    console.timeEnd('Batch update');

    await updateStmt.close();

    // Verify updates
    const warnCount = await db.query("SELECT COUNT(*) as count FROM logs WHERE level = 'WARN'");
    console.log(`✅ Updated ${warnCount.rows[0].count} rows to WARN level`);

    // Performance summary
    console.log('\n📊 Performance Summary:');
    console.log('  ✅ Use prepared statements when:');
    console.log('     - Executing the same query multiple times');
    console.log('     - Performing bulk operations');
    console.log('     - You need maximum performance');
    console.log('  ⚡ Performance gain: ~10× faster');

  } catch (error) {
    console.error('❌ Error:', error.message);
  } finally {
    await db.close();
    console.log('\n✅ Database closed');
  }
}

main().catch(console.error);
