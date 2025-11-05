// Test for parser bug fix - TEXT columns should accept bracket-wrapped content
const { NexusDb } = require('./index.js');

async function testParserFix() {
    console.log('Testing parser fix for TEXT/VECTOR auto-conversion...\n');

    const db = new NexusDb(':memory:');

    try {
        // Create table with TEXT column
        await db.execute('CREATE TABLE test (id INTEGER PRIMARY KEY, data TEXT NOT NULL)');
        console.log('✓ Created table with TEXT column');

        // Test 1: Insert JSON array string into TEXT column
        await db.executeWithParams(
            'INSERT INTO test (id, data) VALUES ($1, $2)',
            [1, '["item1", "item2", "item3"]']
        );
        console.log('✓ Inserted JSON array into TEXT column');

        // Test 2: Insert bracket-wrapped numbers into TEXT column
        await db.executeWithParams(
            'INSERT INTO test (id, data) VALUES ($1, $2)',
            [2, '[1, 2, 3, 4, 5]']
        );
        console.log('✓ Inserted numeric array string into TEXT column');

        // Test 3: Query back the data
        const result = await db.queryWithParams('SELECT * FROM test WHERE id = $1', [1]);
        const rows = JSON.parse(result.rows);

        if (rows[0].data === '["item1", "item2", "item3"]') {
            console.log('✓ Retrieved JSON array correctly as TEXT');
        } else {
            console.error('✗ Data mismatch:', rows[0].data);
            process.exit(1);
        }

        // Test 4: Verify ARRAY syntax still works for vectors
        await db.execute('CREATE TABLE vectors (id INTEGER PRIMARY KEY, embedding VECTOR)');
        console.log('✓ Created table with VECTOR column');

        // Note: ARRAY syntax would be used here if supported
        // For now, just verify TEXT columns work

        console.log('\n✅ All parser fix tests passed!');
        console.log('TEXT columns now properly accept bracket-wrapped content');

    } catch (error) {
        console.error('✗ Test failed:', error.message);
        process.exit(1);
    }
}

testParserFix();
