/**
 * Test for GitHub issue #18
 * insertVector uses positional parameter mapping instead of named columns
 */

const { NexusDb } = require('./wrapper.js');

async function testInsertVectorNamedColumns() {
    const db = new NexusDb(':memory:');

    try {
        // Create table with specific column order: id, title, embedding, author
        await db.execute(`
            CREATE TABLE docs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT,
                embedding VECTOR(3),
                author TEXT
            )
        `);

        console.log('✓ Table created with columns: id, title, embedding, author');

        // Insert with metadata in different order than table schema
        const metadata = { title: 'Doc 1', author: 'Alice' };
        await db.insertVector('docs', [0.1, 0.2, 0.3], JSON.stringify(metadata));

        console.log('✓ Inserted vector with metadata:', metadata);

        // Query the result (using native method to avoid wrapper JSON.parse)
        const result = await db._db.query('SELECT * FROM docs');
        console.log('\nQuery result (raw):');
        console.log('result.rows type:', typeof result.rows);
        console.log('result.rows:', result.rows);

        // Parse if it's a string, otherwise use directly
        const rows = typeof result.rows === 'string' ? JSON.parse(result.rows) : result.rows;
        console.log('\nParsed rows:');
        console.log(JSON.stringify(rows, null, 2));

        // Verify the data
        if (rows.length === 0) {
            console.error('❌ No rows returned');
            process.exit(1);
        }

        const row = rows[0];
        console.log('\nVerification:');
        console.log(`  id: ${row.id} (expected: 1 or auto-generated)`);
        console.log(`  title: ${row.title} (expected: 'Doc 1')`);
        console.log(`  embedding: ${JSON.stringify(row.embedding)} (expected: [0.1, 0.2, 0.3])`);
        console.log(`  author: ${row.author} (expected: 'Alice')`);

        // Check if values match expected
        if (row.title === 'Doc 1' && row.author === 'Alice') {
            console.log('\n✅ SUCCESS: Values correctly mapped to column names!');
        } else {
            console.log('\n❌ FAILURE: Values incorrectly mapped!');
            console.log(`   Expected: title='Doc 1', author='Alice'`);
            console.log(`   Got: title='${row.title}', author='${row.author}'`);
            process.exit(1);
        }

        await db.close();

    } catch (error) {
        console.error('❌ Error:', error.message);
        console.error(error.stack);
        process.exit(1);
    }
}

testInsertVectorNamedColumns();
