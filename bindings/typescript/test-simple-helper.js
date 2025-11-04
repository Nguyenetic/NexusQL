/**
 * Simple test for helper methods
 */

const { NexusDb } = require('./index.js');
const fs = require('fs');

async function test() {
    const dbPath = `test_simple_${Date.now()}.db`;
    console.log('Testing helper methods\n');

    try {
        const db = new NexusDb(dbPath);

        // Create table
        console.log('1. Creating table...');
        await db.execute(`
            CREATE TABLE docs (
                id INTEGER,
                text TEXT,
                embedding VECTOR(3)
            )
        `);
        console.log('   ✅ Table created\n');

        // Test basic insert with SQL
        console.log('2. Testing raw SQL insert...');
        await db.execute(`INSERT INTO docs (id, text, embedding) VALUES (1, 'test', '[0.1, 0.2, 0.3]')`);

        const rawResult = await db.query('SELECT * FROM docs');
        const rawRows = JSON.parse(rawResult.rows);
        console.log('   Raw SQL result:', JSON.stringify(rawRows[0], null, 2));
        console.log('   ✅ Raw SQL works\n');

        // Test insertVector helper
        console.log('3. Testing insertVector...');
        const metadata = JSON.stringify({ id: 2, text: 'helper test' });
        console.log('   Metadata:', metadata);
        await db.insertVector('docs', [0.4, 0.5, 0.6], metadata);

        const helperResult = await db.query('SELECT * FROM docs WHERE id = 2');
        const helperRows = JSON.parse(helperResult.rows);
        console.log('   Helper result:', JSON.stringify(helperRows[0], null, 2));
        console.log('   ✅ insertVector works\n');

        // Test vectorSearch
        console.log('4. Testing vectorSearch...');
        const searchJson = await db.vectorSearch('docs', [0.1, 0.2, 0.3], 2);
        const searchResults = JSON.parse(searchJson);
        console.log('   Search results:', JSON.stringify(searchResults, null, 2));
        console.log('   ✅ vectorSearch works\n');

        await db.close();
        await new Promise(resolve => setTimeout(resolve, 100));
        if (fs.existsSync(dbPath)) {
            fs.unlinkSync(dbPath);
        }

        console.log('🎉 All tests passed!');

    } catch (error) {
        console.error('\n❌ Error:', error.message);
        console.error(error.stack);

        try {
            await new Promise(resolve => setTimeout(resolve, 100));
            if (fs.existsSync(dbPath)) {
                fs.unlinkSync(dbPath);
            }
        } catch (e) {}

        process.exit(1);
    }
}

test();
