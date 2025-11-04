// NexusQL Native Module Test
const { NexusDb } = require('./nexusql.win32-x64-msvc.node');

console.log('=== NexusQL Test ===\n');

async function test() {
    console.log('1. Creating database...');
    const db = new NexusDb('./test_db');
    console.log('✅ Created\n');

    console.log('2. Creating table...');
    await db.execute("CREATE TABLE users (id INT, name TEXT)");
    console.log('✅ Table created\n');

    console.log('3. Inserting data...');
    await db.execute("INSERT INTO users VALUES (1, 'Alice')");
    console.log('✅ Inserted\n');

    console.log('4. Querying...');
    const result = await db.query("SELECT * FROM users");
    console.log('Row count:', result.rowCount);
    console.log('Rows JSON:', result.rows);
    const rows = JSON.parse(result.rows);
    console.log('Parsed rows:', rows);
    console.log();

    console.log('5. Closing...');
    await db.close();
    console.log('✅ Done!\n');

    console.log('🎉 NexusQL is WORKING! Ready for MCP integration!');
}

test().catch(err => {
    console.error('Error:', err);
    process.exit(1);
});
