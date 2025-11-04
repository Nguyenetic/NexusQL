// Comprehensive NexusQL test
const { NexusDb, QueryResult, Transaction } = require('./nexusql.win32-x64-msvc.node');

console.log('=== NexusQL Native Module Test ===\n');

async function test() {
    try {
        // Test 1: Create database
        console.log('1. Creating NexusDB instance...');
        const db = new NexusDb('./test_nexusql_db');
        console.log('✅ Database created!\n');

        // Test 2: Create table
        console.log('2. Creating table...');
        await db.execute('CREATE TABLE IF NOT EXISTS users (id INT PRIMARY KEY, name TEXT, age INT)');
        console.log('✅ Table created!\n');

        // Test 3: Insert data
        console.log('3. Inserting data...');
        const inserted = await db.execute('INSERT INTO users VALUES (1, \'Alice\', 30)');
        console.log(`✅ Inserted ${inserted} row(s)!\n');

        // Test 4: Query data
        console.log('4. Querying data...');
        const result = await db.query('SELECT * FROM users');
        console.log('✅ Query result:');
        console.log('  Row count:', result.rowCount);
        console.log('  Rows (JSON):', result.rows);
        console.log('  Fields:', result.fields);
        console.log();

        // Test 5: Multiple inserts
        console.log('5. Inserting more data...');
        await db.execute("INSERT INTO users VALUES (2, 'Bob', 25)");
        await db.execute("INSERT INTO users VALUES (3, 'Charlie', 35)");
        console.log('✅ More data inserted!\n');

        // Test 6: Query all
        console.log('6. Querying all users...');
        const allUsers = await db.query('SELECT * FROM users ORDER BY id');
        console.log(`✅ Found ${allUsers.rowCount} users:`);
        const rows = JSON.parse(allUsers.rows);
        rows.forEach((row, i) => {
            console.log(`  ${i + 1}. ${JSON.stringify(row)}`);
        });
        console.log();

        // Test 7: Update
        console.log('7. Updating data...');
        const updated = await db.execute("UPDATE users SET age = 26 WHERE name = 'Bob'");
        console.log(`✅ Updated ${updated} row(s)!\n`);

        // Test 8: Delete
        console.log('8. Deleting data...');
        const deleted = await db.execute('DELETE FROM users WHERE id = 3');
        console.log(`✅ Deleted ${deleted} row(s)!\n`);

        // Test 9: Final query
        console.log('9. Final state:');
        const final = await db.query('SELECT * FROM users');
        console.log(`   Total users: ${final.rowCount}`);
        const finalRows = JSON.parse(final.rows);
        finalRows.forEach(row => {
            console.log(`   - ${row.name}, age ${row.age}`);
        });
        console.log();

        // Test 10: Close
        console.log('10. Closing database...');
        await db.close();
        console.log('✅ Database closed!\n');

        console.log('🎉🎉🎉 ALL TESTS PASSED! 🎉🎉🎉');
        console.log('\nNexusQL is ready for hardcore MCP integration testing!');

    } catch (error) {
        console.error('❌ Test failed:', error);
        console.error('Stack:', error.stack);
        process.exit(1);
    }
}

test();
