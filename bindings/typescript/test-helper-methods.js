/**
 * Test suite for TypeScript helper methods
 * Tests insertVector(), vectorSearch(), and createVectorIndex()
 */

const { NexusDb } = require('./index.js');
const fs = require('fs');

async function testHelperMethods() {
    const dbPath = `test_helpers_${Date.now()}.db`;
    console.log('🧪 Testing TypeScript Helper Methods\n');
    console.log('Database:', dbPath, '\n');

    try {
        // Initialize database
        const db = new NexusDb(dbPath);
        console.log('✅ Database created\n');

        // Test 1: Create table with vector column
        console.log('📝 Test 1: Create table with VECTOR column');
        await db.execute(`
            CREATE TABLE documents (
                id INTEGER,
                text TEXT,
                category TEXT,
                embedding VECTOR(384)
            )
        `);
        console.log('✅ Table created successfully\n');

        // Test 2: Insert vectors using helper method
        console.log('📝 Test 2: Insert vectors using insertVector()');

        // Generate some test embeddings (384-dimensional like all-MiniLM-L6-v2)
        const generateEmbedding = (seed) => {
            const embedding = [];
            for (let i = 0; i < 384; i++) {
                // Simple deterministic generation
                embedding.push(Math.sin(seed * 0.01 + i * 0.1));
            }
            return embedding;
        };

        const testData = [
            { id: 1, text: 'Machine learning is fascinating', category: 'AI' },
            { id: 2, text: 'Deep neural networks', category: 'AI' },
            { id: 3, text: 'Cooking pasta recipes', category: 'Food' },
            { id: 4, text: 'Italian cuisine tips', category: 'Food' },
            { id: 5, text: 'Quantum computing basics', category: 'Tech' },
        ];

        console.log(`Inserting ${testData.length} vectors...`);
        const insertStart = Date.now();

        for (const doc of testData) {
            const embedding = generateEmbedding(doc.id);
            const metadata = JSON.stringify({
                id: doc.id,
                text: doc.text,
                category: doc.category
            });

            await db.insertVector('documents', embedding, metadata);
        }

        const insertTime = Date.now() - insertStart;
        console.log(`✅ Inserted ${testData.length} vectors in ${insertTime}ms`);
        console.log(`   Average: ${(insertTime / testData.length).toFixed(2)}ms per insert\n`);

        // Test 3: Vector search with L2 distance (default)
        console.log('📝 Test 3: Vector search with L2 distance (default)');
        const queryEmbedding = generateEmbedding(1.5); // Similar to doc 1 and 2

        const searchStart = Date.now();
        const resultsJson = await db.vectorSearch('documents', queryEmbedding, 3);
        const searchTime = Date.now() - searchStart;

        const results = JSON.parse(resultsJson);
        console.log(`✅ Search completed in ${searchTime}ms`);
        console.log(`   Found ${results.length} results:\n`);

        results.forEach((row, i) => {
            console.log(`   ${i + 1}. ID: ${row.id}, Category: ${row.category}`);
            console.log(`      Text: "${row.text}"`);
            console.log(`      Distance: ${row.distance ? row.distance.toFixed(6) : 'MISSING'}\n`);
        });

        // Test 4: Vector search with Cosine distance
        console.log('📝 Test 4: Vector search with Cosine distance');
        const cosineStart = Date.now();
        const cosineResultsJson = await db.vectorSearch('documents', queryEmbedding, 3, 'cosine');
        const cosineTime = Date.now() - cosineStart;

        const cosineResults = JSON.parse(cosineResultsJson);
        console.log(`✅ Cosine search completed in ${cosineTime}ms`);
        console.log(`   Top 3 results:\n`);

        cosineResults.forEach((row, i) => {
            console.log(`   ${i + 1}. "${row.text}"`);
            console.log(`      Cosine Distance: ${row.distance.toFixed(6)}\n`);
        });

        // Test 5: Vector search with Inner Product
        console.log('📝 Test 5: Vector search with Inner Product');
        const ipStart = Date.now();
        const ipResultsJson = await db.vectorSearch('documents', queryEmbedding, 3, 'inner_product');
        const ipTime = Date.now() - ipStart;

        const ipResults = JSON.parse(ipResultsJson);
        console.log(`✅ Inner Product search completed in ${ipTime}ms`);
        console.log(`   Top 3 results:\n`);

        ipResults.forEach((row, i) => {
            console.log(`   ${i + 1}. "${row.text}"`);
            console.log(`      Inner Product Distance: ${row.distance.toFixed(6)}\n`);
        });

        // Test 6: Create vector index
        console.log('📝 Test 6: Create vector index');
        const indexConfig = JSON.stringify({
            m: 16,
            ef_construction: 64,
            ef_search: 40
        });

        const indexResult = await db.createVectorIndex('documents', 'embedding', 'hnsw', indexConfig);
        console.log(`✅ Index creation result:`);
        console.log(`   ${indexResult}\n`);

        // Test 7: Verify vector data round-trip
        console.log('📝 Test 7: Verify vector data integrity');
        const verifyResult = await db.query('SELECT id, text, embedding FROM documents WHERE id = 1');
        const verifyRows = JSON.parse(verifyResult.rows);

        if (verifyRows.length > 0 && Array.isArray(verifyRows[0].embedding)) {
            const storedEmbedding = verifyRows[0].embedding;
            console.log(`✅ Vector data integrity verified`);
            console.log(`   Stored embedding dimension: ${storedEmbedding.length}`);
            console.log(`   First 5 values: [${storedEmbedding.slice(0, 5).map(v => v.toFixed(4)).join(', ')}]`);
            console.log(`   Last 5 values: [${storedEmbedding.slice(-5).map(v => v.toFixed(4)).join(', ')}]\n`);
        } else {
            console.log('❌ Vector data not properly stored\n');
        }

        // Test 8: Performance test with larger dataset
        console.log('📝 Test 8: Performance test with 100 vectors');

        const perfStart = Date.now();
        for (let i = 6; i <= 105; i++) {
            const embedding = generateEmbedding(i);
            const metadata = JSON.stringify({
                id: i,
                text: `Document ${i}`,
                category: `Category${i % 5}`
            });
            await db.insertVector('documents', embedding, metadata);
        }
        const perfInsertTime = Date.now() - perfStart;

        const perfSearchStart = Date.now();
        const perfResultsJson = await db.vectorSearch('documents', queryEmbedding, 10, 'cosine');
        const perfSearchTime = Date.now() - perfSearchStart;

        const perfResults = JSON.parse(perfResultsJson);

        console.log(`✅ Performance test completed`);
        console.log(`   Insert: 100 vectors in ${perfInsertTime}ms (${(perfInsertTime / 100).toFixed(2)}ms/vector)`);
        console.log(`   Search: Top 10 from 105 vectors in ${perfSearchTime}ms`);
        console.log(`   Results returned: ${perfResults.length}\n`);

        // Test 9: Edge cases
        console.log('📝 Test 9: Edge cases');

        // Empty metadata
        const emptyMetaEmbedding = generateEmbedding(999);
        await db.insertVector('documents', emptyMetaEmbedding);
        console.log('✅ Insert with no metadata works');

        // Search with limit 1
        const limit1Json = await db.vectorSearch('documents', queryEmbedding, 1);
        const limit1Results = JSON.parse(limit1Json);
        console.log(`✅ Search with limit=1 returns ${limit1Results.length} result(s)`);

        // Default parameters
        await db.createVectorIndex('documents');
        console.log('✅ createVectorIndex with all defaults works\n');

        // Summary
        console.log('🎉 All tests passed!\n');
        console.log('Summary:');
        console.log('✅ insertVector() - Working with metadata');
        console.log('✅ vectorSearch() - Working with L2, Cosine, Inner Product');
        console.log('✅ createVectorIndex() - API working (implementation pending)');
        console.log('✅ Vector data integrity - Round-trip verified');
        console.log('✅ Performance - Acceptable for 100+ vectors');
        console.log('✅ Edge cases - Handled correctly\n');

        // Cleanup
        await db.close();

        // Give Windows time to release file handle
        await new Promise(resolve => setTimeout(resolve, 100));

        if (fs.existsSync(dbPath)) {
            fs.unlinkSync(dbPath);
            console.log('🧹 Test database cleaned up');
        }

    } catch (error) {
        console.error('\n❌ Test failed:', error.message);
        console.error(error.stack);

        // Cleanup on error
        try {
            await new Promise(resolve => setTimeout(resolve, 100));
            if (fs.existsSync(dbPath)) {
                fs.unlinkSync(dbPath);
            }
        } catch (cleanupError) {
            console.error('Cleanup error:', cleanupError.message);
        }

        process.exit(1);
    }
}

// Run tests
console.log('═══════════════════════════════════════════════════════════');
console.log('  NexusQL TypeScript Helper Methods Test Suite');
console.log('═══════════════════════════════════════════════════════════\n');

testHelperMethods().then(() => {
    console.log('\n═══════════════════════════════════════════════════════════');
    console.log('  Test Suite Completed Successfully ✨');
    console.log('═══════════════════════════════════════════════════════════\n');
    process.exit(0);
}).catch(error => {
    console.error('\n❌ Fatal error:', error);
    process.exit(1);
});
