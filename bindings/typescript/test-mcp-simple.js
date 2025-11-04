#!/usr/bin/env node
/**
 * Simple MCP Sage compatibility test
 * Tests the core functionality needed for MCP Sage integration
 */

const { NexusDb } = require('./index.js');
const fs = require('fs');

const TEST_DB = `test_mcp_simple_${Date.now()}.db`;

console.log('MCP Sage Simple Compatibility Test\n');

async function test() {
    const db = new NexusDb(TEST_DB);

    try {
        // Create MCP Sage-compatible table
        console.log('1. Creating MCP Sage schema...');
        await db.execute(`
            CREATE TABLE memories (
                id SERIAL PRIMARY KEY,
                user_id TEXT NOT NULL,
                content TEXT NOT NULL,
                memory_level TEXT NOT NULL,
                embedding VECTOR(384),
                metadata TEXT,
                created_at TEXT NOT NULL
            )
        `);
        console.log('✅ Schema created\n');

        // Insert with vector literal (no params)
        console.log('2. Inserting memory with 384-dim vector...');
        const vector = Array(384).fill(0).map((_, i) => Math.sin(i * 0.01));
        const vectorStr = `[${vector.join(',')}]`;

        await db.execute(`
            INSERT INTO memories (user_id, content, memory_level, embedding, metadata, created_at)
            VALUES (
                'test_user',
                'User prefers TypeScript',
                'user',
                '${vectorStr}',
                '{"confidence": 0.95}',
                '2024-11-03T00:00:00Z'
            )
        `);
        console.log('✅ Inserted 1 memory\n');

        // Vector search
        console.log('3. Testing vector search...');
        const queryVec = Array(384).fill(0).map((_, i) => Math.sin(i * 0.01 + 0.1));
        const queryStr = `[${queryVec.join(',')}]`;

        const result = await db.query(`
            SELECT
                id,
                content,
                memory_level,
                embedding <=> '${queryStr}' AS similarity
            FROM memories
            ORDER BY similarity
            LIMIT 1
        `);

        const row = JSON.parse(result.rows)[0];
        console.log('✅ Vector search working!');
        console.log(`   Content: "${row.content}"`);
        console.log(`   Similarity: ${(1 - row.similarity).toFixed(4)}\n`);

        // Test insertVector helper
        console.log('4. Testing insertVector() helper...');
        const vec2 = Array(384).fill(0).map((_, i) => Math.cos(i * 0.01));

        await db.insertVector('memories', vec2, JSON.stringify({
            user_id: 'test_user',
            content: 'User is working on React',
            memory_level: 'session',
            metadata: '{"confidence": 0.85}',
            created_at: '2024-11-03T00:01:00Z'
        }));
        console.log('✅ insertVector() working!\n');

        // Verify all data is there
        const allResult = await db.query('SELECT id, content FROM memories');
        const allRows = JSON.parse(allResult.rows);
        console.log(`5. Total memories: ${allRows.length}`);

        console.log('🎉 All tests passed!');
        console.log('\nNexusQL is ready for MCP Sage integration!');

    } catch (error) {
        console.error('❌ Test failed:', error.message);
        throw error;
    } finally {
        await db.close();
        await new Promise(r => setTimeout(r, 100));
        try {
            if (fs.existsSync(TEST_DB)) {
                fs.unlinkSync(TEST_DB);
            }
        } catch (e) {
            console.log(`Cleanup: ${e.message}`);
        }
    }
}

test().catch(err => {
    console.error(err);
    process.exit(1);
});
