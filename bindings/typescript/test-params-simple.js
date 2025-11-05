/**
 * Simple test to debug parameterized queries
 */

const { NexusDb } = require('./index.js');
const fs = require('fs');

async function test() {
    const dbPath = `test_debug_params_${Date.now()}.db`;

    try {
        const db = new NexusDb(dbPath);

        console.log('Creating table...');
        await db.execute('CREATE TABLE test (id INTEGER, name TEXT)');

        console.log('Testing without parameters...');
        await db.execute("INSERT INTO test VALUES (1, 'Alice')");

        const result1 = await db.query('SELECT * FROM test');
        const rows1 = JSON.parse(result1.rows);
        console.log('Result 1:', JSON.stringify(rows1, null, 2));

        console.log('\nTesting with parameters...');
        try {
            const result2 = await db.executeWithParams(
                'INSERT INTO test VALUES ($1, $2)',
                [2, 'Bob']
            );
            console.log('Insert successful, rows affected:', result2);

            const result3 = await db.query('SELECT * FROM test');
            const rows3 = JSON.parse(result3.rows);
            console.log('Result after parameterized insert:', JSON.stringify(rows3, null, 2));
        } catch (err) {
            console.error('Parameter test failed:', err.message);
            console.error('Full error:', err);
        }

        await db.close();

        // Cleanup
        if (fs.existsSync(dbPath)) {
            fs.unlinkSync(dbPath);
        }

        console.log('\n✓ Test complete');

    } catch (err) {
        console.error('Test failed:', err);
        process.exit(1);
    }
}

test();
