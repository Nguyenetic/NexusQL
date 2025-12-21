/**
 * Comprehensive test to verify which issues are actually broken
 * Tests: ORDER BY, LIMIT, UPDATE, DELETE, parameterized queries, vectors
 */

const { NexusDb } = require('./index.js');

function parseRows(result) {
    if (typeof result.rows === 'string') {
        return JSON.parse(result.rows);
    }
    return result.rows;
}

async function runTests() {
    console.log('=== Comprehensive Issue Testing ===\n');

    const db = new NexusDb(':memory:');
    const results = {};

    try {
        // Setup
        await db.execute(`CREATE TABLE items (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, value INTEGER)`);
        await db.execute(`INSERT INTO items (name, value) VALUES ('Apple', 30)`);
        await db.execute(`INSERT INTO items (name, value) VALUES ('Banana', 10)`);
        await db.execute(`INSERT INTO items (name, value) VALUES ('Cherry', 20)`);
        console.log('Setup: Created table with 3 items\n');

        // Test Issue #36 - ORDER BY
        console.log('Issue #36: ORDER BY');
        try {
            const orderResult = await db.query('SELECT * FROM items ORDER BY value ASC');
            const rows = parseRows(orderResult);
            if (rows && rows.length === 3) {
                const sorted = rows[0].value <= rows[1].value && rows[1].value <= rows[2].value;
                console.log('  Rows returned:', rows.length);
                console.log('  Values:', rows.map(r => r.value));
                console.log('  Sorted correctly:', sorted ? '✅' : '❌');
                results['ORDER BY'] = sorted ? 'PASS' : 'FAIL (not sorted)';
            } else {
                console.log('  ❌ Rows undefined or wrong count');
                results['ORDER BY'] = 'FAIL (undefined)';
            }
        } catch (e) {
            console.log('  ❌ Error:', e.message);
            results['ORDER BY'] = 'ERROR: ' + e.message;
        }

        // Test Issue #36 - LIMIT
        console.log('\nIssue #36: LIMIT');
        try {
            const limitResult = await db.query('SELECT * FROM items LIMIT 2');
            const rows = parseRows(limitResult);
            if (rows && rows.length === 2) {
                console.log('  ✅ LIMIT 2 returned 2 rows');
                results['LIMIT'] = 'PASS';
            } else {
                console.log('  ❌ Expected 2 rows, got:', rows?.length);
                results['LIMIT'] = 'FAIL';
            }
        } catch (e) {
            console.log('  ❌ Error:', e.message);
            results['LIMIT'] = 'ERROR: ' + e.message;
        }

        // Test Issue #37 - Parameterized queries
        console.log('\nIssue #37: Parameterized queries');
        try {
            const paramResult = await db.query('SELECT * FROM items WHERE name = $1', ['Apple']);
            const rows = parseRows(paramResult);
            if (rows && rows.length === 1 && rows[0].name === 'Apple') {
                console.log('  ✅ Parameterized SELECT works');
                results['Parameterized SELECT'] = 'PASS';
            } else {
                console.log('  ❌ Result:', rows);
                results['Parameterized SELECT'] = 'FAIL';
            }
        } catch (e) {
            console.log('  ❌ Error:', e.message);
            results['Parameterized SELECT'] = 'ERROR: ' + e.message;
        }

        // Test NULL parameter
        console.log('\nIssue #37: NULL parameter');
        try {
            await db.execute(`CREATE TABLE nullable (id INTEGER, data TEXT)`);
            await db.execute('INSERT INTO nullable VALUES ($1, $2)', [1, null]);
            const nullResult = await db.query('SELECT * FROM nullable');
            const rows = parseRows(nullResult);
            if (rows && rows.length === 1) {
                console.log('  ✅ NULL parameter inserted, data =', rows[0].data);
                results['NULL parameter'] = 'PASS';
            } else {
                console.log('  ❌ Result:', rows);
                results['NULL parameter'] = 'FAIL';
            }
        } catch (e) {
            console.log('  ❌ Error:', e.message);
            results['NULL parameter'] = 'ERROR: ' + e.message;
        }

        // Test Issue #38 - UPDATE
        console.log('\nIssue #38: UPDATE');
        try {
            await db.execute(`UPDATE items SET value = 100 WHERE name = 'Apple'`);
            const updateResult = await db.query(`SELECT * FROM items WHERE name = 'Apple'`);
            const rows = parseRows(updateResult);
            if (rows && rows.length === 1 && rows[0].value === 100) {
                console.log('  ✅ UPDATE worked, value =', rows[0].value);
                results['UPDATE'] = 'PASS';
            } else {
                console.log('  ❌ Result:', rows);
                results['UPDATE'] = 'FAIL';
            }
        } catch (e) {
            console.log('  ❌ Error:', e.message);
            results['UPDATE'] = 'ERROR: ' + e.message;
        }

        // Test Issue #38 - DELETE
        console.log('\nIssue #38: DELETE');
        try {
            const beforeDelete = await db.query('SELECT COUNT(*) as count FROM items');
            const beforeCount = parseRows(beforeDelete)[0].count;

            await db.execute(`DELETE FROM items WHERE name = 'Banana'`);

            const afterDelete = await db.query('SELECT COUNT(*) as count FROM items');
            const afterCount = parseRows(afterDelete)[0].count;

            if (afterCount === beforeCount - 1) {
                console.log('  ✅ DELETE worked, count before:', beforeCount, 'after:', afterCount);
                results['DELETE'] = 'PASS';
            } else {
                console.log('  ❌ Count before:', beforeCount, 'after:', afterCount);
                results['DELETE'] = 'FAIL';
            }
        } catch (e) {
            console.log('  ❌ Error:', e.message);
            results['DELETE'] = 'ERROR: ' + e.message;
        }

        // Test Issue #40 - Vector operations
        console.log('\nIssue #40: Vector operations');
        try {
            await db.execute(`CREATE TABLE vectors (id INTEGER PRIMARY KEY, embedding VECTOR(3))`);
            await db.execute(`INSERT INTO vectors VALUES (1, '[0.1, 0.2, 0.3]')`);
            const vecResult = await db.query('SELECT * FROM vectors');
            const rows = parseRows(vecResult);
            if (rows && rows.length === 1) {
                console.log('  ✅ Vector insert works, embedding:', rows[0].embedding);
                results['Vector INSERT'] = 'PASS';
            } else {
                console.log('  ❌ Result:', rows);
                results['Vector INSERT'] = 'FAIL';
            }
        } catch (e) {
            console.log('  ❌ Error:', e.message);
            results['Vector INSERT'] = 'ERROR: ' + e.message;
        }

        // Test cosine_distance
        console.log('\nIssue #40: cosine_distance');
        try {
            const distResult = await db.query(
                `SELECT id, cosine_distance(embedding, $1) as dist FROM vectors`,
                [[0.1, 0.2, 0.3]]
            );
            const rows = parseRows(distResult);
            if (rows && rows.length === 1 && rows[0].dist !== null) {
                console.log('  ✅ cosine_distance works, dist =', rows[0].dist);
                results['cosine_distance'] = 'PASS';
            } else {
                console.log('  ❌ Result:', rows);
                results['cosine_distance'] = 'FAIL';
            }
        } catch (e) {
            console.log('  ❌ Error:', e.message);
            results['cosine_distance'] = 'ERROR: ' + e.message;
        }

        await db.close();

        // Summary
        console.log('\n========================================');
        console.log('SUMMARY:');
        console.log('========================================');
        let passed = 0, failed = 0;
        for (const [test, result] of Object.entries(results)) {
            const status = result === 'PASS' ? '✅' : '❌';
            console.log(`${status} ${test}: ${result}`);
            if (result === 'PASS') passed++; else failed++;
        }
        console.log(`\nTotal: ${passed} passed, ${failed} failed`);

    } catch (e) {
        console.error('Test setup failed:', e);
        await db.close();
    }
}

runTests().catch(console.error);
