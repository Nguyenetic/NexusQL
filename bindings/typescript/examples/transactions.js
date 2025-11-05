/**
 * Transactions Example
 *
 * Demonstrates ACID transactions with commit and rollback
 */

const { NexusDb } = require('../index.js');

async function main() {
  console.log('💳 ACID Transactions Example\n');

  const db = new NexusDb(':memory:');

  try {
    // Setup bank accounts
    await db.execute(`
      CREATE TABLE accounts (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        balance REAL NOT NULL
      )
    `);

    await db.execute('INSERT INTO accounts VALUES (1, "Alice", 1000)');
    await db.execute('INSERT INTO accounts VALUES (2, "Bob", 500)');

    console.log('Initial balances:');
    let balances = await db.query('SELECT * FROM accounts ORDER BY id');
    console.table(balances.rows);

    // Example 1: Successful transaction (commit)
    console.log('\n1️⃣  Transferring $100 from Alice to Bob...');
    const tx1 = await db.beginTransaction();

    try {
      await tx1.query('UPDATE accounts SET balance = balance - 100 WHERE id = 1');
      await tx1.query('UPDATE accounts SET balance = balance + 100 WHERE id = 2');
      await tx1.commit();
      console.log('✅ Transaction committed successfully');
    } catch (error) {
      await tx1.rollback();
      console.error('❌ Transaction rolled back:', error.message);
    }

    console.log('\nBalances after transfer:');
    balances = await db.query('SELECT * FROM accounts ORDER BY id');
    console.table(balances.rows);

    // Example 2: Failed transaction (rollback)
    console.log('\n2️⃣  Attempting invalid transfer (should rollback)...');
    const tx2 = await db.beginTransaction();

    try {
      await tx2.query('UPDATE accounts SET balance = balance - 2000 WHERE id = 1'); // Alice doesn't have enough
      await tx2.query('UPDATE accounts SET balance = balance + 2000 WHERE id = 2');
      await tx2.query('INSERT INTO invalid_table VALUES (1)'); // This will fail
      await tx2.commit();
    } catch (error) {
      await tx2.rollback();
      console.log('✅ Transaction rolled back (expected):', error.message);
    }

    console.log('\nBalances after failed transfer (unchanged):');
    balances = await db.query('SELECT * FROM accounts ORDER BY id');
    console.table(balances.rows);

    // Example 3: Multi-step transaction
    console.log('\n3️⃣  Multi-step transaction (transfer + audit log)...');

    await db.execute(`
      CREATE TABLE transaction_log (
        id INTEGER PRIMARY KEY,
        from_account INTEGER,
        to_account INTEGER,
        amount REAL,
        timestamp TEXT DEFAULT CURRENT_TIMESTAMP
      )
    `);

    const tx3 = await db.beginTransaction();

    try {
      await tx3.query('UPDATE accounts SET balance = balance - 50 WHERE id = 2');
      await tx3.query('UPDATE accounts SET balance = balance + 50 WHERE id = 1');
      await tx3.query(`INSERT INTO transaction_log (id, from_account, to_account, amount)
                       VALUES (1, 2, 1, 50)`);
      await tx3.commit();
      console.log('✅ Multi-step transaction committed');
    } catch (error) {
      await tx3.rollback();
      console.error('❌ Transaction rolled back:', error.message);
    }

    console.log('\nFinal balances:');
    balances = await db.query('SELECT * FROM accounts ORDER BY id');
    console.table(balances.rows);

    console.log('\nTransaction log:');
    const logs = await db.query('SELECT * FROM transaction_log');
    console.table(logs.rows);

    console.log('\n📊 Summary:');
    console.log('  - Transaction 1: ✅ Committed (Alice → Bob $100)');
    console.log('  - Transaction 2: ❌ Rolled back (invalid operation)');
    console.log('  - Transaction 3: ✅ Committed (Bob → Alice $50 + audit)');

  } catch (error) {
    console.error('❌ Error:', error.message);
  } finally {
    await db.close();
    console.log('\n✅ Database closed');
  }
}

main().catch(console.error);
