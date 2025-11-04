const { NexusDb } = require('./index.js');

async function test() {
    const db = new NexusDb('test_func.db');

    await db.execute(`CREATE TABLE test (id INTEGER, val TEXT)`);
    await db.execute(`INSERT INTO test VALUES (1, 'hello')`);

    console.log('\n1. Basic SELECT:');
    const basic = await db.query('SELECT * FROM test');
    console.log(JSON.parse(basic.rows));

    console.log('\n2. SELECT with UPPER function:');
    const withFunc = await db.query('SELECT *, UPPER(val) as upper_val FROM test');
    console.log(JSON.parse(withFunc.rows));

    console.log('\n3. SELECT with literal AS:');
    const withLiteral = await db.query(`SELECT *, 'test' as test_col FROM test`);
    console.log(JSON.parse(withLiteral.rows));

    await db.close();
}

test().catch(console.error);
