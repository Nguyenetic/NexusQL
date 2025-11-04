/**
 * Debug test to understand NexusDb API structure
 */

const { NexusDb } = require('./index.js');
const fs = require('fs');
const path = require('path');

async function runTest() {
    console.log('=== NexusQL API Debug Test ===\n');

    const dbPath = path.join(__dirname, 'test_db', 'debug_test.db');
    const dbDir = path.dirname(dbPath);

    if (fs.existsSync(dbPath)) {
        fs.unlinkSync(dbPath);
    }

    if (!fs.existsSync(dbDir)) {
        fs.mkdirSync(dbDir, { recursive: true });
    }

    const db = new NexusDb(dbPath);
    console.log('✓ Database created\n');

    try {
        // Create table
        await db.execute(`
            CREATE TABLE test (
                id INTEGER PRIMARY KEY,
                name TEXT,
                value REAL
            )
        `);
        console.log('✓ Table created\n');

        // Insert data
        await db.execute("INSERT INTO test (id, name, value) VALUES (1, 'Alice', 1.5)");
        await db.execute("INSERT INTO test (id, name, value) VALUES (2, 'Bob', 2.7)");
        console.log('✓ Data inserted\n');

        // Query and inspect structure
        const result = await db.query('SELECT * FROM test');

        console.log('=== Query Result Structure ===');
        console.log('Result object:', result);
        console.log('\nResult type:', typeof result);
        console.log('Result constructor:', result.constructor.name);
        console.log('\nrowCount:', result.rowCount);
        console.log('rowCount type:', typeof result.rowCount);

        console.log('\nrows (raw):', result.rows);
        console.log('rows type:', typeof result.rows);
        console.log('rows length:', result.rows ? result.rows.length : 'N/A');

        console.log('\nfields:', result.fields);
        console.log('fields type:', typeof result.fields);
        console.log('fields length:', result.fields ? result.fields.length : 'N/A');

        if (result.fields && result.fields.length > 0) {
            console.log('\nFirst field structure:', result.fields[0]);
        }

        // Try to parse rows
        console.log('\n=== Attempting to Parse Rows ===');
        try {
            const parsed = JSON.parse(result.rows);
            console.log('✓ Rows parsed successfully');
            console.log('Parsed type:', typeof parsed);
            console.log('Parsed structure:', parsed);
            console.log('Is Array:', Array.isArray(parsed));

            if (Array.isArray(parsed) && parsed.length > 0) {
                console.log('\nFirst row:', parsed[0]);
                console.log('First row keys:', Object.keys(parsed[0]));
            }
        } catch (err) {
            console.log('✗ Failed to parse rows:', err.message);
            console.log('Raw rows string (first 200 chars):', result.rows.substring(0, 200));
        }

    } catch (err) {
        console.error('\n✗ Error:', err.message);
        console.error(err.stack);
    } finally {
        await db.close();
        console.log('\n✓ Database closed');
    }
}

runTest().catch(console.error);
