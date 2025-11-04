const { NexusDb } = require('./index.js');

async function test() {
    const db = new NexusDb('test_debug.db');

    await db.execute(`
        CREATE TABLE test (
            id INTEGER,
            embedding VECTOR(3)
        )
    `);

    await db.execute(`INSERT INTO test VALUES (1, '[0.1, 0.2, 0.3]')`);
    await db.execute(`INSERT INTO test VALUES (2, '[0.4, 0.5, 0.6]')`);

    console.log('\n1. Basic SELECT:');
    const basic = await db.query('SELECT * FROM test');
    console.log(JSON.stringify(JSON.parse(basic.rows), null, 2));

    console.log('\n2. With vector_distance function:');
    const withDist = await db.query(`SELECT *, vector_distance(embedding, '[0.1, 0.2, 0.3]') as distance FROM test`);
    console.log(JSON.stringify(JSON.parse(withDist.rows), null, 2));

    console.log('\n3. Using vectorSearch helper:');
    const helperJson = await db.vectorSearch('test', [0.1, 0.2, 0.3], 2);
    console.log(JSON.stringify(JSON.parse(helperJson), null, 2));

    await db.close();

    console.log('\n✅ Debug complete');
}

test().catch(console.error);
