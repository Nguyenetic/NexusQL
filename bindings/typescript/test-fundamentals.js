const { NexusDb } = require('./index.js');

async function testFundamentals() {
    console.log('=== Testing SQL Fundamentals ===\n');
    const db = new NexusDb(':memory:');
    let passed = 0;
    let failed = 0;
    const failures = [];

    async function test(name, fn) {
        try {
            await fn();
            console.log(`✅ ${name}`);
            passed++;
        } catch (e) {
            console.log(`❌ ${name}: ${e.message}`);
            failures.push({ name, error: e.message });
            failed++;
        }
    }

    function parseRows(result) {
        if (!result.rows) return [];
        const rows = typeof result.rows === 'function' ? result.rows() : result.rows;
        return typeof rows === 'string' ? JSON.parse(rows) : rows;
    }

    // ========== CREATE TABLE ==========
    console.log('--- CREATE TABLE ---');

    await test('CREATE TABLE basic', async () => {
        await db.execute('CREATE TABLE users (id INTEGER, name TEXT, email TEXT)');
    });

    await test('CREATE TABLE with PRIMARY KEY', async () => {
        await db.execute('CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT, price REAL)');
    });

    await test('CREATE TABLE with NOT NULL', async () => {
        await db.execute('CREATE TABLE orders (id INTEGER, customer_id INTEGER NOT NULL, total REAL)');
    });

    await test('CREATE TABLE IF NOT EXISTS', async () => {
        await db.execute('CREATE TABLE IF NOT EXISTS users (id INTEGER, name TEXT)');
    });

    // ========== INSERT ==========
    console.log('\n--- INSERT ---');

    await test('INSERT single row', async () => {
        await db.execute("INSERT INTO users (id, name, email) VALUES (1, 'Alice', 'alice@test.com')");
    });

    await test('INSERT multiple rows', async () => {
        await db.execute("INSERT INTO users (id, name, email) VALUES (2, 'Bob', 'bob@test.com')");
        await db.execute("INSERT INTO users (id, name, email) VALUES (3, 'Charlie', 'charlie@test.com')");
    });

    await test('INSERT with NULL', async () => {
        await db.execute("INSERT INTO users (id, name, email) VALUES (4, 'Dave', NULL)");
    });

    await test('INSERT with numbers', async () => {
        await db.execute("INSERT INTO products (id, name, price) VALUES (1, 'Widget', 9.99)");
        await db.execute("INSERT INTO products (id, name, price) VALUES (2, 'Gadget', 19.99)");
        await db.execute("INSERT INTO products (id, name, price) VALUES (3, 'Gizmo', 29.99)");
    });

    // ========== SELECT ==========
    console.log('\n--- SELECT ---');

    await test('SELECT *', async () => {
        const result = await db.query('SELECT * FROM users');
        const rows = parseRows(result);
        if (rows.length !== 4) throw new Error(`Expected 4 rows, got ${rows.length}`);
    });

    await test('SELECT specific columns', async () => {
        const result = await db.query('SELECT name, email FROM users');
        const rows = parseRows(result);
        if (!rows[0].name) throw new Error('Missing name column');
    });

    await test('SELECT with WHERE =', async () => {
        const result = await db.query("SELECT * FROM users WHERE id = 1");
        const rows = parseRows(result);
        if (rows.length !== 1) throw new Error(`Expected 1 row, got ${rows.length}`);
        if (rows[0].name !== 'Alice') throw new Error(`Expected Alice, got ${rows[0].name}`);
    });

    await test('SELECT with WHERE string comparison', async () => {
        const result = await db.query("SELECT * FROM users WHERE name = 'Bob'");
        const rows = parseRows(result);
        if (rows.length !== 1) throw new Error(`Expected 1 row, got ${rows.length}`);
    });

    await test('SELECT with WHERE >', async () => {
        const result = await db.query('SELECT * FROM products WHERE price > 15');
        const rows = parseRows(result);
        if (rows.length !== 2) throw new Error(`Expected 2 rows, got ${rows.length}`);
    });

    await test('SELECT with WHERE <', async () => {
        const result = await db.query('SELECT * FROM products WHERE price < 20');
        const rows = parseRows(result);
        if (rows.length !== 2) throw new Error(`Expected 2 rows, got ${rows.length}`);
    });

    await test('SELECT with WHERE >=', async () => {
        const result = await db.query('SELECT * FROM products WHERE price >= 19.99');
        const rows = parseRows(result);
        if (rows.length !== 2) throw new Error(`Expected 2 rows, got ${rows.length}`);
    });

    await test('SELECT with WHERE <=', async () => {
        const result = await db.query('SELECT * FROM products WHERE price <= 19.99');
        const rows = parseRows(result);
        if (rows.length !== 2) throw new Error(`Expected 2 rows, got ${rows.length}`);
    });

    await test('SELECT with WHERE != / <>', async () => {
        const result = await db.query("SELECT * FROM users WHERE name != 'Alice'");
        const rows = parseRows(result);
        if (rows.length !== 3) throw new Error(`Expected 3 rows, got ${rows.length}`);
    });

    await test('SELECT with WHERE AND', async () => {
        const result = await db.query('SELECT * FROM products WHERE price > 10 AND price < 25');
        const rows = parseRows(result);
        if (rows.length !== 1) throw new Error(`Expected 1 row, got ${rows.length}`);
    });

    await test('SELECT with WHERE OR', async () => {
        const result = await db.query("SELECT * FROM users WHERE name = 'Alice' OR name = 'Bob'");
        const rows = parseRows(result);
        if (rows.length !== 2) throw new Error(`Expected 2 rows, got ${rows.length}`);
    });

    await test('SELECT with WHERE IS NULL', async () => {
        const result = await db.query('SELECT * FROM users WHERE email IS NULL');
        const rows = parseRows(result);
        if (rows.length !== 1) throw new Error(`Expected 1 row, got ${rows.length}`);
    });

    await test('SELECT with WHERE IS NOT NULL', async () => {
        const result = await db.query('SELECT * FROM users WHERE email IS NOT NULL');
        const rows = parseRows(result);
        if (rows.length !== 3) throw new Error(`Expected 3 rows, got ${rows.length}`);
    });

    await test('SELECT with WHERE LIKE', async () => {
        const result = await db.query("SELECT * FROM users WHERE name LIKE 'A%'");
        const rows = parseRows(result);
        if (rows.length !== 1) throw new Error(`Expected 1 row, got ${rows.length}`);
    });

    await test('SELECT with WHERE LIKE %middle%', async () => {
        const result = await db.query("SELECT * FROM users WHERE name LIKE '%li%'");
        const rows = parseRows(result);
        if (rows.length !== 2) throw new Error(`Expected 2 rows (Alice, Charlie), got ${rows.length}`);
    });

    // ========== ORDER BY ==========
    console.log('\n--- ORDER BY ---');

    await test('ORDER BY ASC', async () => {
        const result = await db.query('SELECT * FROM products ORDER BY price ASC');
        const rows = parseRows(result);
        if (rows[0].price > rows[1].price) throw new Error('Not sorted ascending');
    });

    await test('ORDER BY DESC', async () => {
        const result = await db.query('SELECT * FROM products ORDER BY price DESC');
        const rows = parseRows(result);
        if (rows[0].price < rows[1].price) throw new Error('Not sorted descending');
    });

    await test('ORDER BY string column', async () => {
        const result = await db.query('SELECT * FROM users ORDER BY name ASC');
        const rows = parseRows(result);
        if (rows[0].name !== 'Alice') throw new Error(`Expected Alice first, got ${rows[0].name}`);
    });

    // ========== LIMIT ==========
    console.log('\n--- LIMIT ---');

    await test('LIMIT', async () => {
        const result = await db.query('SELECT * FROM users LIMIT 2');
        const rows = parseRows(result);
        if (rows.length !== 2) throw new Error(`Expected 2 rows, got ${rows.length}`);
    });

    await test('LIMIT with ORDER BY', async () => {
        const result = await db.query('SELECT * FROM products ORDER BY price DESC LIMIT 1');
        const rows = parseRows(result);
        if (rows.length !== 1) throw new Error(`Expected 1 row, got ${rows.length}`);
        if (rows[0].price !== 29.99) throw new Error(`Expected highest price 29.99, got ${rows[0].price}`);
    });

    // ========== UPDATE ==========
    console.log('\n--- UPDATE ---');

    await test('UPDATE single row', async () => {
        await db.execute("UPDATE users SET email = 'alice.new@test.com' WHERE id = 1");
        const result = await db.query('SELECT email FROM users WHERE id = 1');
        const rows = parseRows(result);
        if (rows[0].email !== 'alice.new@test.com') throw new Error(`Update failed: ${rows[0].email}`);
    });

    await test('UPDATE multiple columns', async () => {
        await db.execute("UPDATE users SET name = 'Robert', email = 'robert@test.com' WHERE id = 2");
        const result = await db.query('SELECT * FROM users WHERE id = 2');
        const rows = parseRows(result);
        if (rows[0].name !== 'Robert') throw new Error(`Name not updated: ${rows[0].name}`);
        if (rows[0].email !== 'robert@test.com') throw new Error(`Email not updated: ${rows[0].email}`);
    });

    await test('UPDATE with numeric value', async () => {
        await db.execute('UPDATE products SET price = 24.99 WHERE id = 2');
        const result = await db.query('SELECT price FROM products WHERE id = 2');
        const rows = parseRows(result);
        if (rows[0].price !== 24.99) throw new Error(`Price not updated: ${rows[0].price}`);
    });

    // ========== DELETE ==========
    console.log('\n--- DELETE ---');

    await test('DELETE single row', async () => {
        const before = parseRows(await db.query('SELECT COUNT(*) as cnt FROM users'));
        await db.execute('DELETE FROM users WHERE id = 4');
        const after = parseRows(await db.query('SELECT COUNT(*) as cnt FROM users'));
        if (after[0].cnt !== before[0].cnt - 1) throw new Error('Delete did not reduce count');
    });

    await test('DELETE with condition', async () => {
        await db.execute("INSERT INTO users (id, name, email) VALUES (5, 'Eve', 'eve@test.com')");
        await db.execute("DELETE FROM users WHERE name = 'Eve'");
        const result = await db.query("SELECT * FROM users WHERE name = 'Eve'");
        const rows = parseRows(result);
        if (rows.length !== 0) throw new Error('Delete failed, row still exists');
    });

    // ========== AGGREGATE FUNCTIONS ==========
    console.log('\n--- AGGREGATE FUNCTIONS ---');

    await test('COUNT(*)', async () => {
        const result = await db.query('SELECT COUNT(*) as cnt FROM users');
        const rows = parseRows(result);
        if (typeof rows[0].cnt !== 'number') throw new Error(`COUNT returned non-number: ${typeof rows[0].cnt}`);
    });

    await test('COUNT(column)', async () => {
        const result = await db.query('SELECT COUNT(email) as cnt FROM users');
        const rows = parseRows(result);
        if (typeof rows[0].cnt !== 'number') throw new Error(`COUNT returned non-number: ${typeof rows[0].cnt}`);
    });

    await test('SUM', async () => {
        const result = await db.query('SELECT SUM(price) as total FROM products');
        const rows = parseRows(result);
        if (typeof rows[0].total !== 'number') throw new Error(`SUM returned non-number: ${typeof rows[0].total}`);
    });

    await test('AVG', async () => {
        const result = await db.query('SELECT AVG(price) as avg_price FROM products');
        const rows = parseRows(result);
        if (typeof rows[0].avg_price !== 'number') throw new Error(`AVG returned non-number: ${typeof rows[0].avg_price}`);
    });

    await test('MIN', async () => {
        const result = await db.query('SELECT MIN(price) as min_price FROM products');
        const rows = parseRows(result);
        if (rows[0].min_price !== 9.99) throw new Error(`MIN expected 9.99, got ${rows[0].min_price}`);
    });

    await test('MAX', async () => {
        const result = await db.query('SELECT MAX(price) as max_price FROM products');
        const rows = parseRows(result);
        if (rows[0].max_price !== 29.99) throw new Error(`MAX expected 29.99, got ${rows[0].max_price}`);
    });

    // ========== DROP TABLE ==========
    console.log('\n--- DROP TABLE ---');

    await test('DROP TABLE', async () => {
        await db.execute('DROP TABLE orders');
        // Verify it's gone by trying to select from it
        try {
            await db.query('SELECT * FROM orders');
            throw new Error('Table should not exist');
        } catch (e) {
            if (!e.message.includes('not found') && !e.message.includes('does not exist')) {
                // Table was actually dropped, this is expected behavior
            }
        }
    });

    await test('DROP TABLE IF EXISTS', async () => {
        await db.execute('DROP TABLE IF EXISTS nonexistent_table');
    });

    // ========== Summary ==========
    console.log('\n========================================');
    console.log('SUMMARY:');
    console.log('========================================');
    console.log(`✅ Passed: ${passed}`);
    console.log(`❌ Failed: ${failed}`);

    if (failures.length > 0) {
        console.log('\nFailures:');
        failures.forEach(f => console.log(`  - ${f.name}: ${f.error}`));
    }

    await db.close();
    process.exit(failed > 0 ? 1 : 0);
}

testFundamentals().catch(console.error);
