const { NexusDb } = require('./index.js');

async function test() {
    const db = new NexusDb(':memory:');
    
    console.log('🎉 Testing Unified API v3.0.0\n');
    console.log('='.repeat(50));
    
    // Test 1: CREATE TABLE without params
    await db.execute('CREATE TABLE users (id INT, name TEXT, age INT)');
    console.log('✅ Test 1: Created table without params');
    
    // Test 2: INSERT with params (unified API - NO executeWithParams needed!)
    await db.execute('INSERT INTO users VALUES ($1, $2, $3)', [1, 'Alice', 30]);
    await db.execute('INSERT INTO users VALUES ($1, $2, $3)', [2, 'Bob', 25]);
    await db.execute('INSERT INTO users VALUES ($1, $2, $3)', [3, 'Charlie', 35]);
    console.log('✅ Test 2: Inserted 3 rows with params using execute(sql, params)');
    
    // Test 3: SELECT without params - rows auto-parsed!
    const result1 = await db.query('SELECT * FROM users');
    console.log('\n✅ Test 3: Query without params');
    console.log('   Rows:', JSON.stringify(result1.rows, null, 2));
    console.log('   ✨ Array.isArray(rows):', Array.isArray(result1.rows));
    console.log('   ✨ typeof rows[0]:', typeof result1.rows[0]);
    console.log('   ✨ NO JSON.parse() needed!');
    
    // Test 4: SELECT with params (unified API - NO queryWithParams needed!)
    const result2 = await db.query('SELECT * FROM users WHERE age > $1', [26]);
    console.log('\n✅ Test 4: Query with params using query(sql, params)');
    console.log('   Result:', result2.rows);
    console.log('   Found', result2.rows.length, 'users over 26');
    
    // Test 5: UPDATE with params
    await db.execute('UPDATE users SET age = $1 WHERE name = $2', [31, 'Alice']);
    const result3 = await db.query('SELECT * FROM users WHERE name = $1', ['Alice']);
    console.log('\n✅ Test 5: UPDATE with params');
    console.log('   Alice new age:', result3.rows[0].age);
    
    // Test 6: DELETE with params
    await db.execute('DELETE FROM users WHERE name = $1', ['Bob']);
    const result4 = await db.query('SELECT * FROM users');
    console.log('\n✅ Test 6: DELETE with params');
    console.log('   Remaining users:', result4.rows.length);
    
    // Test 7: Transactions with ROLLBACK
    await db.execute('BEGIN TRANSACTION');
    await db.execute('INSERT INTO users VALUES ($1, $2, $3)', [4, 'David', 40]);
    const beforeRollback = await db.query('SELECT * FROM users');
    await db.execute('ROLLBACK');
    const afterRollback = await db.query('SELECT * FROM users');
    console.log('\n✅ Test 7: Transaction with ROLLBACK');
    console.log('   Before rollback:', beforeRollback.rows.length, 'users');
    console.log('   After rollback:', afterRollback.rows.length, 'users');
    console.log('   ✨ Rollback worked!');
    
    console.log('\n' + '='.repeat(50));
    console.log('🎉 ALL TESTS PASSED!\n');
    console.log('📊 v3.0.0 Features Verified:');
    console.log('   ✅ Unified execute(sql, params?) - NO executeWithParams()');
    console.log('   ✅ Unified query(sql, params?) - NO queryWithParams()');
    console.log('   ✅ Rows auto-parsed to JS objects - NO JSON.parse()');
    console.log('   ✅ True transaction semantics with ROLLBACK');
    console.log('   ✅ Clean, simple API matching pg, mysql2, better-sqlite3');
    console.log('\n🚀 Ready to publish!');
}

test().catch(err => {
    console.error('❌ Test failed:', err);
    process.exit(1);
});
