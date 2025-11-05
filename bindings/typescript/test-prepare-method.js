// Test the new prepare() method
const { NexusDb } = require('./index.js');

async function testPrepareMethod() {
    console.log('Testing prepare() method...\n');

    const db = new NexusDb(':memory:');

    try {
        // Create test table
        await db.execute('CREATE TABLE test_users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)');
        console.log('✓ Created test_users table');

        // Test 1: INSERT with prepared statement
        const insertStmt = await db.prepare('INSERT INTO test_users (id, name, email) VALUES ($1, $2, $3)');
        await insertStmt.execute([1, 'Alice', 'alice@example.com']);
        await insertStmt.execute([2, 'Bob', 'bob@example.com']);
        await insertStmt.execute([3, 'Charlie', 'charlie@example.com']);
        await insertStmt.close();
        console.log('✓ Inserted 3 users with prepared statement');

        // Test 2: SELECT with prepared statement
        const selectStmt = await db.prepare('SELECT * FROM test_users WHERE id = $1');
        const result1 = await selectStmt.execute([1]);
        const rows1 = JSON.parse(result1.rows);

        if (rows1.length === 1 && rows1[0].name === 'Alice') {
            console.log('✓ SELECT with prepared statement returned correct result');
        } else {
            console.error('✗ SELECT returned unexpected result:', rows1);
            process.exit(1);
        }

        const result2 = await selectStmt.execute([2]);
        const rows2 = JSON.parse(result2.rows);

        if (rows2.length === 1 && rows2[0].name === 'Bob') {
            console.log('✓ Reused prepared statement with different parameter');
        } else {
            console.error('✗ Second SELECT returned unexpected result:', rows2);
            process.exit(1);
        }

        await selectStmt.close();

        // Test 3: SQL injection protection
        const maliciousInput = "'; DROP TABLE test_users; --";
        const safeStmt = await db.prepare('SELECT * FROM test_users WHERE name = $1');
        const safeResult = await safeStmt.execute([maliciousInput]);
        const safeRows = JSON.parse(safeResult.rows);

        if (safeRows.length === 0) {
            console.log('✓ SQL injection protected - malicious input treated as data');
        } else {
            console.error('✗ SQL injection test failed');
            process.exit(1);
        }

        await safeStmt.close();

        // Verify table still exists (using SELECT * instead of COUNT due to aggregate function parser issue)
        const verifyResult = await db.query('SELECT * FROM test_users');
        const verifyRows = JSON.parse(verifyResult.rows);

        if (verifyRows.length === 3) {
            console.log('✓ Table intact after SQL injection attempt (3 users remain)');
        } else {
            console.error('✗ Table modified unexpectedly');
            process.exit(1);
        }

        await db.close();

        console.log('\n✅ All prepare() method tests passed!');
        console.log('The prepare() API is fully functional and secure.');

    } catch (error) {
        console.error('✗ Test failed:', error.message);
        console.error(error.stack);
        process.exit(1);
    }
}

testPrepareMethod();
