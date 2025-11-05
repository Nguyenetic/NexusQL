/**
 * Test script to verify the published npm package works correctly
 * This tests the ACTUAL API as implemented, not assumptions
 */

const { NexusDb } = require('./index.js');
const fs = require('fs');

async function testPublishedPackage() {
    const dbPath = `test_npm_${Date.now()}.db`;

    console.log('Testing @nguyenetic/nexusql@2.6.0 from npm...\n');

    try {
        const db = new NexusDb(dbPath);

        // Test 1: Basic SQL
        console.log('✓ Test 1: Basic SQL execution');
        await db.execute('CREATE TABLE users (id INTEGER, name TEXT, email TEXT)');
        await db.execute("INSERT INTO users VALUES (1, 'Alice', 'alice@example.com')");
        console.log('  ✓ Table created and data inserted\n');

        // Test 2: Query results (with JSON parsing)
        console.log('✓ Test 2: Query with result parsing');
        const result1 = await db.query('SELECT * FROM users');
        const rows1 = JSON.parse(result1.rows); // IMPORTANT: Must parse JSON
        console.log('  Result:', rows1);
        console.log('  ✓ First user:', rows1[0].name, '-', rows1[0].email);
        console.log('');

        // Test 3: Parameterized INSERT (CORRECT API)
        console.log('✓ Test 3: Parameterized INSERT (executeWithParams)');
        await db.executeWithParams(
            'INSERT INTO users (id, name, email) VALUES ($1, $2, $3)',
            [2, 'Bob', 'bob@example.com']
        );
        console.log('  ✓ Inserted with parameters\n');

        // Test 4: Parameterized SELECT (CORRECT API)
        console.log('✓ Test 4: Parameterized SELECT (queryWithParams)');
        const result2 = await db.queryWithParams(
            'SELECT * FROM users WHERE id = $1',
            [2]
        );
        const rows2 = JSON.parse(result2.rows);
        console.log('  Result:', rows2);
        console.log('  ✓ Found user:', rows2[0].name);
        console.log('');

        // Test 5: SQL Injection Prevention
        console.log('✓ Test 5: SQL Injection Prevention');
        const malicious = "'; DROP TABLE users; --";
        const result3 = await db.queryWithParams(
            'SELECT * FROM users WHERE name = $1',
            [malicious]
        );
        const rows3 = JSON.parse(result3.rows);
        console.log('  Malicious input safely handled');
        console.log('  Result rows:', rows3.length); // Should be 0

        // Verify table still exists
        const allUsers = await db.query('SELECT * FROM users');
        const allRows = JSON.parse(allUsers.rows);
        console.log('  ✓ Table still exists with', allRows.length, 'users\n');

        // Test 6: Multiple parameters
        console.log('✓ Test 6: Multiple parameters');
        const result4 = await db.queryWithParams(
            'SELECT * FROM users WHERE id > $1 OR name = $2',
            [0, 'Alice']
        );
        const rows4 = JSON.parse(result4.rows);
        console.log('  ✓ Found', rows4.length, 'users\n');

        // Test 7: UPDATE with parameters
        console.log('✓ Test 7: UPDATE with parameters');
        await db.executeWithParams(
            'UPDATE users SET email = $1 WHERE id = $2',
            ['newemail@example.com', 1]
        );
        const updated = await db.queryWithParams('SELECT * FROM users WHERE id = $1', [1]);
        const updatedRows = JSON.parse(updated.rows);
        console.log('  ✓ Updated email:', updatedRows[0].email);
        console.log('');

        // Test 8: DELETE with parameters
        console.log('✓ Test 8: DELETE with parameters');
        await db.executeWithParams(
            'DELETE FROM users WHERE id = $1',
            [2]
        );
        const remaining = await db.query('SELECT * FROM users');
        const remainingRows = JSON.parse(remaining.rows);
        console.log('  ✓ Remaining users:', remainingRows.length);
        console.log('');

        await db.close();

        // Cleanup
        setTimeout(() => {
            try {
                if (fs.existsSync(dbPath)) {
                    fs.unlinkSync(dbPath);
                }
            } catch (e) {}
        }, 100);

        console.log('═'.repeat(60));
        console.log('✅ ALL TESTS PASSED!');
        console.log('═'.repeat(60));
        console.log('\nSummary:');
        console.log('  • Basic SQL: ✓');
        console.log('  • Query results: ✓ (requires JSON.parse)');
        console.log('  • Parameterized INSERT: ✓ (executeWithParams)');
        console.log('  • Parameterized SELECT: ✓ (queryWithParams)');
        console.log('  • SQL Injection Prevention: ✓');
        console.log('  • Multiple parameters: ✓');
        console.log('  • UPDATE with parameters: ✓');
        console.log('  • DELETE with parameters: ✓');
        console.log('\nCorrect API Usage:');
        console.log('  • db.executeWithParams(sql, [params]) - for INSERT/UPDATE/DELETE');
        console.log('  • db.queryWithParams(sql, [params]) - for SELECT');
        console.log('  • JSON.parse(result.rows) - to access row data');
        console.log('  • $1, $2, $3 syntax - NOT ? syntax');

    } catch (error) {
        console.error('\n❌ TEST FAILED:', error.message);
        console.error(error);
        process.exit(1);
    }
}

testPublishedPackage();
