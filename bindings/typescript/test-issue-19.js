/**
 * Test for GitHub issue #19
 * Parameterized queries map values to wrong columns (CRITICAL)
 */

const { NexusDb } = require('./wrapper.js');

async function testParameterizedQueryColumnMapping() {
    const db = new NexusDb(':memory:');

    try {
        // Create table with AUTO_INCREMENT id
        await db.execute(`CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT,
            age INTEGER,
            email TEXT
        )`);

        console.log('✓ Table created with columns: id, name, age, email');

        // Insert with parameterized query
        await db.execute(
            'INSERT INTO users (name, age, email) VALUES ($1, $2, $3)',
            ['Alice', 25, 'alice@example.com']
        );

        console.log('✓ Inserted with parameterized query: name=Alice, age=25, email=alice@example.com');

        // Query the result (using native method to handle serde_json::Value)
        const result = await db._db.query('SELECT * FROM users');
        const rows = typeof result.rows === 'string' ? JSON.parse(result.rows) : result.rows;
        console.log('\nQuery result:');
        console.log(JSON.stringify(rows, null, 2));

        // Verify the data
        if (rows.length === 0) {
            console.error('❌ No rows returned');
            process.exit(1);
        }

        const row = rows[0];
        console.log('\nVerification:');
        console.log(`  id: ${row.id} (expected: null or auto-generated)`);
        console.log(`  name: ${row.name} (expected: 'Alice')`);
        console.log(`  age: ${row.age} (expected: 25)`);
        console.log(`  email: ${row.email} (expected: 'alice@example.com')`);

        // Check if values match expected
        if (row.name === 'Alice' && row.age === 25 && row.email === 'alice@example.com') {
            console.log('\n✅ SUCCESS: Parameterized query values correctly mapped to column names!');
            console.log('   Issue #19 is FIXED!');
        } else {
            console.log('\n❌ FAILURE: Values incorrectly mapped!');
            console.log(`   Expected: name='Alice', age=25, email='alice@example.com'`);
            console.log(`   Got: name='${row.name}', age=${row.age}, email='${row.email}'`);
            process.exit(1);
        }

        await db.close();

    } catch (error) {
        console.error('❌ Error:', error.message);
        console.error(error.stack);
        process.exit(1);
    }
}

testParameterizedQueryColumnMapping();
