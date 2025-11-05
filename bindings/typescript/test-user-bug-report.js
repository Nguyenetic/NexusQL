// Test to reproduce the exact bugs from NEXUSQL_2.7.0_BUG_VERIFICATION.md
const { NexusDb } = require('./index.js');

async function testUserBugs() {
    console.log('NexusQL Version: 2.7.0');
    console.log('Testing user-reported bugs...\n');

    const db = new NexusDb(':memory:');

    // Create table
    await db.execute('CREATE TABLE users (id INTEGER, name TEXT, email TEXT)');
    console.log('✓ Table created\n');

    // Test 1: Parameterized queries
    console.log('--- Test 1: Parameterized Queries ---');
    try {
        // User is calling execute() with parameters - but execute() doesn't take parameters!
        // They should use executeWithParams()
        await db.execute('INSERT INTO users VALUES ($1, $2, $3)', [1, 'Alice', 'alice@test.com']);
        console.log('✓ Parameterized INSERT worked!');
    } catch (err) {
        console.log('✗ Parameterized INSERT failed:', err.message);
        console.log('NOTE: execute() does not accept parameters. Use executeWithParams() instead.');

        // Try the correct API
        try {
            await db.executeWithParams('INSERT INTO users VALUES ($1, $2, $3)', [1, 'Alice', 'alice@test.com']);
            console.log('✓ Parameterized INSERT worked with executeWithParams()!');
        } catch (err2) {
            console.log('✗ executeWithParams() also failed:', err2.message);
        }
    }

    // Test 2: Query result format
    console.log('\n--- Test 2: Query Result Format ---');
    try {
        const result = await db.query('SELECT * FROM users');
        console.log('Result type:', typeof result.rows);
        console.log('Is array:', Array.isArray(result.rows));
        console.log('Raw result:', result.rows);

        if (typeof result.rows === 'string') {
            console.log('⚠️  Result is a JSON string (by design)');
            console.log('Parsing...');
            const parsed = JSON.parse(result.rows);
            console.log('Parsed result:', parsed);
            console.log('Is parsed array:', Array.isArray(parsed));
        } else if (Array.isArray(result.rows)) {
            console.log('✓ Result is proper array!');
        }
    } catch (err) {
        console.log('✗ Query failed:', err.message);
    }

    // Test 3: Data integrity
    console.log('\n--- Test 3: Data Integrity ---');
    try {
        const result = await db.query('SELECT * FROM users WHERE id = 1');
        console.log('Raw result:', result.rows);

        const rows = JSON.parse(result.rows);
        console.log('Parsed rows:', rows);

        if (rows.length > 0) {
            const row = rows[0];
            console.log('Row data:', row);
            if (row.name === 'Alice' && row.email === 'alice@test.com') {
                console.log('✓ Data stored correctly!');
            } else {
                console.log('✗ Data corruption detected!');
                console.log('Expected: name=Alice, email=alice@test.com');
                console.log('Got:', row);
            }
        } else {
            console.log('✗ No rows returned!');
        }
    } catch (err) {
        console.log('✗ Data check failed:', err.message);
    }

    // Test 4: Transactions (API method)
    console.log('\n--- Test 4: Transactions (using beginTransaction API) ---');
    try {
        const tx = await db.beginTransaction();
        await tx.query('INSERT INTO users VALUES (2, \'Bob\', \'bob@test.com\')');
        await tx.commit();
        console.log('✓ Transaction worked with beginTransaction() API!');
    } catch (err) {
        console.log('✗ Transaction failed:', err.message);
    }

    // Test 5: Transactions (SQL)
    console.log('\n--- Test 5: Transactions (using SQL BEGIN/COMMIT) ---');
    try {
        await db.execute('BEGIN TRANSACTION');
        await db.execute('INSERT INTO users VALUES (3, \'Charlie\', \'charlie@test.com\')');
        await db.execute('COMMIT');
        console.log('✓ SQL transactions worked!');
    } catch (err) {
        console.log('✗ SQL transactions failed:', err.message);
        console.log('NOTE: Use db.beginTransaction() API instead of SQL BEGIN/COMMIT');
    }

    console.log('\n=== SUMMARY ===');
    const finalResult = await db.query('SELECT * FROM users');
    const finalRows = JSON.parse(finalResult.rows);
    console.log('Total rows:', finalRows.length);
    console.log('All data:', finalRows);

    await db.close();
}

testUserBugs().catch(console.error);
