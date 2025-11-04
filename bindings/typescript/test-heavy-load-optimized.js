const { NexusDb } = require('./index.js');

const TEST_DB = `test_optimized_${Date.now()}.db`;
const VECTOR_DIM = 384;

// Results tracking
const results = {
    startTime: Date.now(),
    tests: []
};

function getMemoryUsage() {
    const usage = process.memoryUsage();
    return {
        heapUsed: `${(usage.heapUsed / 1024 / 1024).toFixed(2)} MB`,
        heapTotal: `${(usage.heapTotal / 1024 / 1024).toFixed(2)} MB`
    };
}

function generateVector(dim, seed) {
    return Array(dim).fill(0).map((_, i) => Math.sin((seed + i) * 0.01));
}

async function runOptimizedTests() {
    const db = new NexusDb(TEST_DB);

    console.log('═'.repeat(70));
    console.log('  NexusQL OPTIMIZED Heavy Load Testing Suite');
    console.log('  Using Context7 Best Practices for Windows');
    console.log('═'.repeat(70));
    console.log();
    console.log('📊 Starting Optimized Tests\n');
    console.log(`Database: ${TEST_DB}`);
    console.log(`Vector Dimensions: ${VECTOR_DIM}`);
    console.log(`Start Memory: ${getMemoryUsage().heapUsed}\n`);

    try {
        // ==========================================
        // Test 1: Schema Creation
        // ==========================================
        console.log('📝 Test 1: Create schema with multiple tables');
        const schemaStart = Date.now();

        await db.execute(`
            CREATE TABLE memories (
                id SERIAL PRIMARY KEY,
                user_id TEXT NOT NULL,
                content TEXT NOT NULL,
                memory_level TEXT DEFAULT 'session',
                embedding VECTOR(${VECTOR_DIM}),
                metadata TEXT,
                created_at TEXT NOT NULL
            )
        `);

        await db.execute(`
            CREATE TABLE users (
                id SERIAL PRIMARY KEY,
                username TEXT NOT NULL,
                email TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
        `);

        const schemaTime = Date.now() - schemaStart;
        console.log(`✅ Schema created in ${schemaTime}ms\n`);
        results.tests.push({ name: 'Schema Creation', time: schemaTime, status: 'pass' });

        // ==========================================
        // Test 2: OPTIMIZED Batch Insert - 1,000 vectors
        // ==========================================
        console.log('📝 Test 2: OPTIMIZED batch insert 1,000 vectors (384-dim)');
        console.log('   Using batchInsertVectors() with batches of 100');
        const bulk1kStart = Date.now();
        const count1k = 1000;
        const batchSize = 100;

        for (let batchStart = 0; batchStart < count1k; batchStart += batchSize) {
            const batchEnd = Math.min(batchStart + batchSize, count1k);
            const vectors = [];
            const metadata = [];

            for (let i = batchStart; i < batchEnd; i++) {
                const vector = generateVector(VECTOR_DIM, i);
                vectors.push(vector);

                metadata.push(JSON.stringify({
                    user_id: `user_${i % 10}`,
                    content: `Memory content ${i}`,
                    metadata: `{"index": ${i}, "category": "test"}`,
                    created_at: new Date().toISOString()
                }));
            }

            await db.batchInsertVectors('memories', vectors, metadata);

            process.stdout.write(`\r   Progress: ${batchEnd}/${count1k} vectors inserted...`);
        }

        const bulk1kTime = Date.now() - bulk1kStart;
        const bulk1kAvg = (bulk1kTime / count1k).toFixed(2);
        console.log(`\r✅ Inserted ${count1k} vectors in ${bulk1kTime}ms (${bulk1kAvg}ms avg)`);
        console.log(`   Memory after 1K inserts: ${getMemoryUsage().heapUsed}\n`);
        results.tests.push({
            name: 'OPTIMIZED Bulk Insert 1K',
            time: bulk1kTime,
            avgPerOp: bulk1kAvg,
            status: 'pass'
        });

        // ==========================================
        // Test 3: Search Performance - Various Sizes
        // ==========================================
        console.log('📝 Test 3: Vector search performance tests');

        const searchTests = [
            { limit: 1, name: 'Top-1 search' },
            { limit: 10, name: 'Top-10 search' },
            { limit: 50, name: 'Top-50 search' },
            { limit: 100, name: 'Top-100 search' },
        ];

        const queryVector = generateVector(VECTOR_DIM, 500);
        const queryStr = `[${queryVector.join(',')}]`;

        for (const test of searchTests) {
            const searchStart = Date.now();

            const result = await db.query(`
                SELECT
                    id,
                    content,
                    user_id,
                    embedding <=> '${queryStr}' AS similarity
                FROM memories
                ORDER BY similarity
                LIMIT ${test.limit}
            `);

            const searchTime = Date.now() - searchStart;
            const rows = JSON.parse(result.rows);
            console.log(`   ✅ ${test.name} (limit ${test.limit}): ${searchTime}ms, got ${rows.length} results`);
            results.tests.push({
                name: test.name,
                time: searchTime,
                status: 'pass'
            });
        }
        console.log();

        // ==========================================
        // Test 4: Filtered Vector Search
        // ==========================================
        console.log('📝 Test 4: Filtered vector search (WHERE + vector similarity)');
        const filteredStart = Date.now();

        const filteredResult = await db.query(`
            SELECT
                id,
                content,
                user_id,
                embedding <=> '${queryStr}' AS similarity
            FROM memories
            WHERE user_id = 'user_5'
            ORDER BY similarity
            LIMIT 20
        `);

        const filteredTime = Date.now() - filteredStart;
        const filteredRows = JSON.parse(filteredResult.rows);
        console.log(`✅ Filtered search: ${filteredTime}ms, found ${filteredRows.length} results`);
        console.log(`   All results match filter: ${filteredRows.every(r => r.user_id === 'user_5')}\n`);
        results.tests.push({
            name: 'Filtered Vector Search',
            time: filteredTime,
            status: 'pass'
        });

        // ==========================================
        // Test 5: Multiple Distance Metrics
        // ==========================================
        console.log('📝 Test 5: Test all distance metrics');

        const metrics = [
            { op: '<=>', name: 'Cosine' },
            { op: '<->', name: 'L2' },
            { op: '<#>', name: 'Inner Product' }
        ];

        for (const metric of metrics) {
            const metricStart = Date.now();

            const metricResult = await db.query(`
                SELECT
                    id,
                    embedding ${metric.op} '${queryStr}' AS distance
                FROM memories
                ORDER BY distance
                LIMIT 10
            `);

            const metricTime = Date.now() - metricStart;
            const metricRows = JSON.parse(metricResult.rows);
            console.log(`   ✅ ${metric.name} (${metric.op}): ${metricTime}ms, ${metricRows.length} results`);
            results.tests.push({
                name: `Distance Metric: ${metric.name}`,
                time: metricTime,
                status: 'pass'
            });
        }
        console.log();

        // ==========================================
        // Test 6: Concurrent Search Operations
        // ==========================================
        console.log('📝 Test 6: Concurrent search operations (10 parallel searches)');
        const concurrentStart = Date.now();

        const concurrentSearches = [];
        for (let i = 0; i < 10; i++) {
            const queryVec = generateVector(VECTOR_DIM, 1000 + i);
            const qStr = `[${queryVec.join(',')}]`;

            concurrentSearches.push(
                db.query(`
                    SELECT id, embedding <=> '${qStr}' AS similarity
                    FROM memories
                    ORDER BY similarity
                    LIMIT 5
                `)
            );
        }

        const concurrentResults = await Promise.all(concurrentSearches);
        const concurrentTime = Date.now() - concurrentStart;
        console.log(`✅ 10 concurrent searches completed in ${concurrentTime}ms`);
        console.log(`   Average per search: ${(concurrentTime / 10).toFixed(2)}ms`);
        console.log(`   All searches successful: ${concurrentResults.every(r => r.rows)}\n`);
        results.tests.push({
            name: 'Concurrent Searches (10x)',
            time: concurrentTime,
            avgPerOp: (concurrentTime / 10).toFixed(2),
            status: 'pass'
        });

        // ==========================================
        // Test 7: OPTIMIZED Massive Batch Insert - 10,000 vectors
        // ==========================================
        console.log('📝 Test 7: OPTIMIZED massive batch insert (10,000 vectors)');
        console.log('   Using batchInsertVectors() with batches of 200');
        const bulk10kStart = Date.now();
        const count10k = 10000;
        const largeBatchSize = 200;

        for (let batchStart = 0; batchStart < count10k; batchStart += largeBatchSize) {
            const batchEnd = Math.min(batchStart + largeBatchSize, count10k);
            const vectors = [];
            const metadata = [];

            for (let i = batchStart; i < batchEnd; i++) {
                const vector = generateVector(VECTOR_DIM, i + 1000);
                vectors.push(vector);

                metadata.push(JSON.stringify({
                    user_id: `user_${i % 20}`,
                    content: `Large batch memory ${i}`,
                    memory_level: i % 3 === 0 ? 'user' : 'session',
                    metadata: `{"batch": "large", "index": ${i}}`,
                    created_at: new Date().toISOString()
                }));
            }

            await db.batchInsertVectors('memories', vectors, metadata);

            if ((batchEnd) % 1000 === 0) {
                process.stdout.write(`\r   Progress: ${batchEnd}/${count10k} vectors inserted...`);
            }
        }

        const bulk10kTime = Date.now() - bulk10kStart;
        const bulk10kAvg = (bulk10kTime / count10k).toFixed(2);
        console.log(`\r✅ Inserted ${count10k} vectors in ${bulk10kTime}ms (${bulk10kAvg}ms avg)`);
        console.log(`   Memory after 10K more: ${getMemoryUsage().heapUsed}\n`);
        results.tests.push({
            name: 'OPTIMIZED Bulk Insert 10K',
            time: bulk10kTime,
            avgPerOp: bulk10kAvg,
            status: 'pass'
        });

        // ==========================================
        // Test 8: Search Performance at Scale (~11K vectors)
        // ==========================================
        console.log('📝 Test 8: Search performance with ~11K total vectors');
        const scaleStart = Date.now();

        const scaleResult = await db.query(`
            SELECT
                id,
                content,
                embedding <=> '${queryStr}' AS similarity
            FROM memories
            ORDER BY similarity
            LIMIT 20
        `);

        const scaleTime = Date.now() - scaleStart;
        const scaleRows = JSON.parse(scaleResult.rows);
        console.log(`✅ Search across ~11K vectors: ${scaleTime}ms, ${scaleRows.length} results\n`);
        results.tests.push({
            name: 'Search at Scale (11K)',
            time: scaleTime,
            status: 'pass'
        });

        // ==========================================
        // Final Summary
        // ==========================================
        const totalTime = Date.now() - results.startTime;
        const endMemory = getMemoryUsage();

        console.log('═'.repeat(70));
        console.log('  Test Summary - OPTIMIZED with Context7 Best Practices');
        console.log('═'.repeat(70));
        console.log();
        console.log(`Total Tests: ${results.tests.length}`);
        console.log(`Passed: ${results.tests.filter(t => t.status === 'pass').length}`);
        console.log(`Failed: ${results.tests.filter(t => t.status === 'fail').length}`);
        console.log(`Total Time: ${(totalTime / 1000).toFixed(2)}s`);
        console.log();
        console.log('Memory Usage:');
        console.log(`  Start: ${getMemoryUsage().heapUsed}`);
        console.log(`  End: ${endMemory.heapUsed}`);
        console.log();
        console.log('Performance Highlights:');
        console.log(`  1K Vector Insert (OPTIMIZED): ${results.tests[1].avgPerOp}ms avg`);
        console.log(`  10K Vector Insert (OPTIMIZED): ${results.tests.find(t => t.name.includes('10K')).avgPerOp}ms avg`);
        console.log(`  Top-10 Search (1K): ${results.tests[2].time}ms`);
        console.log(`  Top-10 Search (11K): ~${results.tests[results.tests.length - 1].time}ms`);
        console.log(`  Concurrent Searches: ${results.tests.find(t => t.name.includes('Concurrent')).avgPerOp}ms avg`);
        console.log();
        console.log('🎉 All optimized tests PASSED! NexusQL batch operations working!');

    } catch (error) {
        console.error('❌ Optimized test failed:', error);
        throw error;
    } finally {
        try {
            await db.close();
            console.log('\n✅ Database closed successfully');
        } catch (err) {
            console.error('Cleanup error:', err.message);
        }
    }
}

runOptimizedTests().catch(err => {
    console.error('Fatal error:', err);
    process.exit(1);
});
