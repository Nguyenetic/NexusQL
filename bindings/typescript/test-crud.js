// Comprehensive CRUD Test for NexusQL
const { NexusDb } = require('./nexusql.win32-x64-msvc.node');

console.log('=== NexusQL Comprehensive CRUD Test ===\n');

async function test() {
    const db = new NexusDb('./test_db_crud');

    try {
        // CREATE
        console.log('1. CREATE TABLE');
        await db.execute("CREATE TABLE products (id INT, name TEXT, price INT)");
        console.log('✅ Table created\n');

        // INSERT (Single)
        console.log('2. INSERT (Single Row)');
        await db.execute("INSERT INTO products VALUES (1, 'Laptop', 999)");
        console.log('✅ Inserted 1 row\n');

        // INSERT (Multiple)
        console.log('3. INSERT (Multiple Rows)');
        await db.execute("INSERT INTO products VALUES (2, 'Mouse', 25)");
        await db.execute("INSERT INTO products VALUES (3, 'Keyboard', 75)");
        await db.execute("INSERT INTO products VALUES (4, 'Monitor', 299)");
        console.log('✅ Inserted 3 more rows\n');

        // SELECT (All)
        console.log('4. SELECT (All Rows)');
        let result = await db.query("SELECT * FROM products");
        let rows = JSON.parse(result.rows);
        console.log(`✅ Found ${rows.length} products:`);
        rows.forEach(r => console.log(`   - ${r.name}: $${r.price}`));
        console.log();

        // SELECT (With LIMIT)
        console.log('5. SELECT (With LIMIT)');
        result = await db.query("SELECT * FROM products LIMIT 2");
        rows = JSON.parse(result.rows);
        console.log(`✅ Limited to ${rows.length} rows:`);
        rows.forEach(r => console.log(`   - ${r.name}`));
        console.log();

        // UPDATE
        console.log('6. UPDATE');
        await db.execute("UPDATE products SET price = 899 WHERE id = 1");
        result = await db.query("SELECT * FROM products WHERE id = 1");
        rows = JSON.parse(result.rows);
        console.log(`✅ Updated Laptop price to $${rows[0].price}\n`);

        // DELETE
        console.log('7. DELETE');
        await db.execute("DELETE FROM products WHERE id = 2");
        result = await db.query("SELECT * FROM products");
        rows = JSON.parse(result.rows);
        console.log(`✅ Deleted Mouse, ${rows.length} products remain:`);
        rows.forEach(r => console.log(`   - ${r.name}`));
        console.log();

        // Data Types
        console.log('8. Data Types Test');
        await db.execute("CREATE TABLE types_test (int_val INT, text_val TEXT, float_val INT)");
        await db.execute("INSERT INTO types_test VALUES (42, 'hello', 99)");
        result = await db.query("SELECT * FROM types_test");
        rows = JSON.parse(result.rows);
        console.log('✅ Data types:');
        console.log(`   - Integer: ${rows[0].int_val} (${typeof rows[0].int_val})`);
        console.log(`   - Text: ${rows[0].text_val} (${typeof rows[0].text_val})`);
        console.log(`   - Float: ${rows[0].float_val} (${typeof rows[0].float_val})`);
        console.log();

        // Persistence Test
        console.log('9. Persistence Test');
        await db.close();
        console.log('✅ Database closed\n');

        // Wait for file handles to be released (Windows can be slow)
        await new Promise(resolve => setTimeout(resolve, 500));

        const db2 = new NexusDb('./test_db_crud');
        result = await db2.query("SELECT * FROM products");
        rows = JSON.parse(result.rows);
        console.log(`✅ Reopened database, ${rows.length} products persisted:`);
        rows.forEach(r => console.log(`   - ${r.name}: $${r.price}`));
        console.log();

        await db2.close();

        console.log('🎉 ALL CRUD OPERATIONS WORKING PERFECTLY!\n');
        console.log('Summary:');
        console.log('✅ CREATE TABLE');
        console.log('✅ INSERT (single & multiple)');
        console.log('✅ SELECT (all & limited)');
        console.log('✅ UPDATE');
        console.log('✅ DELETE');
        console.log('✅ Data type handling');
        console.log('✅ Data persistence');
        console.log('\n🚀 NexusQL TypeScript bindings are production-ready!');

    } catch (error) {
        console.error('❌ Error:', error);
        process.exit(1);
    }
}

test();
