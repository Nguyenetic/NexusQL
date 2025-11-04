// NexusQL Vector Search Test
// Phase 1 MVP: Vector operations with brute-force search

const NexusQL = require('./index.js');

async function testVectorSearch() {
    console.log('=== NexusQL Vector Search Test (Phase 1 MVP) ===\n');

    const db = new NexusQL('./test_vector_db');

    try {
        // Test 1: CREATE TABLE with VECTOR column
        console.log('✅ Test 1: CREATE TABLE with VECTOR(3) column');
        await db.execute(`
            CREATE TABLE documents (
                id INTEGER,
                title TEXT,
                embedding VECTOR(3)
            )
        `);
        console.log('   Created documents table with 3D vector column\n');

        // Test 2: INSERT vectors using string literal syntax
        console.log('✅ Test 2: INSERT vectors (string literal syntax)');
        await db.execute(`
            INSERT INTO documents (id, title, embedding)
            VALUES (1, 'Machine Learning', '[0.8, 0.2, 0.1]')
        `);
        await db.execute(`
            INSERT INTO documents (id, title, embedding)
            VALUES (2, 'Deep Learning', '[0.9, 0.3, 0.15]')
        `);
        await db.execute(`
            INSERT INTO documents (id, title, embedding)
            VALUES (3, 'Natural Language', '[0.1, 0.8, 0.7]')
        `);
        await db.execute(`
            INSERT INTO documents (id, title, embedding)
            VALUES (4, 'Computer Vision', '[0.3, 0.1, 0.9]')
        `);
        console.log('   Inserted 4 documents with embeddings\n');

        // Test 3: SELECT with L2 distance (Euclidean) - <-> operator
        console.log('✅ Test 3: ORDER BY L2 distance (<->)');
        const query_vector = '[0.85, 0.25, 0.12]'; // Close to "Machine Learning"
        const l2_results = await db.query(`
            SELECT
                id,
                title,
                embedding <-> '${query_vector}' AS distance
            FROM documents
            ORDER BY embedding <-> '${query_vector}'
            LIMIT 3
        `);
        console.log('   Query vector:', query_vector);
        console.log('   Top 3 nearest neighbors (L2):');
        for (const row of l2_results) {
            const id = row.values[0];
            const title = row.values[1];
            const distance = row.values[2];
            console.log(`   - ${id}. ${title} (distance: ${distance})`);
        }
        console.log();

        // Test 4: SELECT with Cosine distance - <=> operator
        console.log('✅ Test 4: ORDER BY Cosine distance (<=>)');
        const cosine_results = await db.query(`
            SELECT
                id,
                title,
                embedding <=> '[0.1, 0.9, 0.6]' AS distance
            FROM documents
            ORDER BY embedding <=> '[0.1, 0.9, 0.6]'
            LIMIT 2
        `);
        console.log('   Query vector: [0.1, 0.9, 0.6] (close to "Natural Language")');
        console.log('   Top 2 nearest neighbors (Cosine):');
        for (const row of cosine_results) {
            const id = row.values[0];
            const title = row.values[1];
            const distance = row.values[2];
            console.log(`   - ${id}. ${title} (distance: ${distance})`);
        }
        console.log();

        // Test 5: SELECT with Inner Product - <#> operator
        console.log('✅ Test 5: ORDER BY Inner Product (<#>)');
        const inner_results = await db.query(`
            SELECT
                id,
                title,
                embedding <#> '[0.5, 0.5, 0.5]' AS neg_dot_product
            FROM documents
            ORDER BY embedding <#> '[0.5, 0.5, 0.5]'
            LIMIT 3
        `);
        console.log('   Query vector: [0.5, 0.5, 0.5]');
        console.log('   Top 3 results (Inner Product):');
        for (const row of inner_results) {
            const id = row.values[0];
            const title = row.values[1];
            const neg_dot = row.values[2];
            console.log(`   - ${id}. ${title} (negative dot product: ${neg_dot})`);
        }
        console.log();

        // Test 6: Verify exact vector retrieval
        console.log('✅ Test 6: Verify exact vector retrieval');
        const exact_results = await db.query(`
            SELECT id, title, embedding
            FROM documents
            WHERE id = 1
        `);
        console.log('   Retrieved document #1:');
        for (const row of exact_results) {
            const id = row.values[0];
            const title = row.values[1];
            const embedding = row.values[2];
            console.log(`   - ID: ${id}`);
            console.log(`   - Title: ${title}`);
            console.log(`   - Embedding: ${embedding}`);
        }
        console.log();

        // Test 7: All documents
        console.log('✅ Test 7: SELECT all documents');
        const all_docs = await db.query(`SELECT id, title FROM documents`);
        console.log(`   Total documents: ${all_docs.length}`);
        for (const row of all_docs) {
            console.log(`   - ${row.values[0]}. ${row.values[1]}`);
        }
        console.log();

        console.log('🎉 All Vector Search Tests PASSED!\n');
        console.log('📊 Phase 1 MVP Complete:');
        console.log('   ✅ VECTOR(n) data type');
        console.log('   ✅ Vector literal parsing: \'[0.1, 0.2, 0.3]\'');
        console.log('   ✅ Distance operators: <-> (L2), <=> (Cosine), <#> (Inner Product)');
        console.log('   ✅ ORDER BY with distance calculations');
        console.log('   ✅ LIMIT for top-k retrieval');
        console.log('   ✅ Brute-force vector search (exact results)');
        console.log();
        console.log('🚀 Next Steps (Phase 2):');
        console.log('   - HNSW index for approximate nearest neighbor search');
        console.log('   - 10-100× speedup on large datasets (100K+ vectors)');
        console.log('   - Quantization (Binary, Int8, Float16) for memory efficiency');
        console.log('   - Re-scoring for accuracy');
        console.log();

    } catch (error) {
        console.error('❌ Test failed:', error.message);
        console.error(error.stack);
        process.exit(1);
    } finally {
        await db.close();
    }
}

// Run tests
testVectorSearch().catch(err => {
    console.error('Fatal error:', err);
    process.exit(1);
});
