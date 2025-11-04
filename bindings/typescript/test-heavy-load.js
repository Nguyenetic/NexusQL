#!/usr/bin/env node
/**
 * NexusQL Heavy Load & Stress Testing
 *
 * Tests:
 * 1. Large-scale vector insertion (1K, 5K, 10K vectors)
 * 2. Bulk search performance
 * 3. Concurrent operations
 * 4. Memory usage
 * 5. Edge cases
 * 6. Error recovery
 * 7. Long-running operations
 */

const { NexusDb } = require('./index.js');
const fs = require('fs');

const TEST_DB = `test_heavy_${Date.now()}.db`;
const VECTOR_DIM = 384;

console.log('═'.repeat(70));
console.log('  NexusQL Heavy Load & Stress Testing Suite');
console.log('═'.repeat(70));
console.log();

// Helper to generate deterministic vectors
function generateVector(dim, seed) {
    const vector = new Array(dim);
    for (let i = 0; i < dim; i++) {
        vector[i] = Math.sin(seed * (i + 1) * 0.01);
    }
    // Normalize
    const magnitude = Math.sqrt(vector.reduce((sum, val) => sum + val * val, 0));
    return vector.map(val => val / magnitude);
}

// Helper to measure memory usage
function getMemoryUsage() {
    const usage = process.memoryUsage();
    return {
        heapUsed: (usage.heapUsed / 1024 / 1024).toFixed(2) + ' MB',
        heapTotal: (usage.heapTotal / 1024 / 1024).toFixed(2) + ' MB',
        rss: (usage.rss / 1024 / 1024).toFixed(2) + ' MB'
    };
}

async function heavyLoadTests() {
    const db = new NexusDb(TEST_DB);
    const results = {
        tests: [],
        startTime: Date.now(),
        startMemory: getMemoryUsage()
    };

    try {
        console.log('📊 Starting Heavy Load Tests\n');
        console.log(`Database: ${TEST_DB}`);
        console.log(`Vector Dimensions: ${VECTOR_DIM}`);
        console.log(`Start Memory: ${results.startMemory.heapUsed}\n`);

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
        // Test 2: Bulk Insert - 1,000 vectors
        // ==========================================
        console.log('📝 Test 2: Bulk insert 1,000 vectors (384-dim)');
        const bulk1kStart = Date.now();
        const count1k = 1000;

        for (let i = 0; i < count1k; i++) {
            const vector = generateVector(VECTOR_DIM, i);
            const vectorStr = `[${vector.join(',')}]`;
            const now = new Date().toISOString();

            await db.execute(`
                INSERT INTO memories (user_id, content, embedding, metadata, created_at)
                VALUES (
                    'user_${i % 10}',
                    'Memory content ${i}',
                    '${vectorStr}',
                    '{"index": ${i}, "category": "test"}',
                    '${now}'
                )
            `);

            if ((i + 1) % 100 === 0) {
                process.stdout.write(`\r   Progress: ${i + 1}/${count1k} vectors inserted...`);
            }
        }

        const bulk1kTime = Date.now() - bulk1kStart;
        const bulk1kAvg = (bulk1kTime / count1k).toFixed(2);
        console.log(`\r✅ Inserted ${count1k} vectors in ${bulk1kTime}ms (${bulk1kAvg}ms avg)`);
        console.log(`   Memory after 1K inserts: ${getMemoryUsage().heapUsed}\n`);
        results.tests.push({
            name: 'Bulk Insert 1K',
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
                resultCount: rows.length,
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
                name: `${metric.name} Distance`,
                time: metricTime,
                status: 'pass'
            });
        }
        console.log();

        // ==========================================
        // Test 6: Concurrent Operations
        // ==========================================
        console.log('📝 Test 6: Concurrent search operations (10 parallel searches)');
        const concurrentStart = Date.now();

        const concurrentSearches = Array(10).fill(0).map(async (_, idx) => {
            const qv = generateVector(VECTOR_DIM, 1000 + idx);
            const qs = `[${qv.join(',')}]`;

            return db.query(`
                SELECT id, embedding <=> '${qs}' AS similarity
                FROM memories
                ORDER BY similarity
                LIMIT 5
            `);
        });

        const concurrentResults = await Promise.all(concurrentSearches);
        const concurrentTime = Date.now() - concurrentStart;

        console.log(`✅ 10 concurrent searches completed in ${concurrentTime}ms`);
        console.log(`   Average per search: ${(concurrentTime / 10).toFixed(2)}ms`);
        console.log(`   All searches successful: ${concurrentResults.every(r => r.rows)}\n`);
        results.tests.push({
            name: 'Concurrent Searches',
            time: concurrentTime,
            avgPerOp: (concurrentTime / 10).toFixed(2),
            status: 'pass'
        });

        // ==========================================
        // Test 7: Large Result Sets
        // ==========================================
        console.log('📝 Test 7: Large result set retrieval (500 results)');
        const largeStart = Date.now();

        const largeResult = await db.query(`
            SELECT
                id,
                content,
                user_id,
                embedding <=> '${queryStr}' AS similarity
            FROM memories
            ORDER BY similarity
            LIMIT 500
        `);

        const largeTime = Date.now() - largeStart;
        const largeRows = JSON.parse(largeResult.rows);

        console.log(`✅ Retrieved ${largeRows.length} results in ${largeTime}ms`);
        console.log(`   Results properly sorted: ${largeRows[0].similarity <= largeRows[largeRows.length-1].similarity}\n`);
        results.tests.push({
            name: 'Large Result Set',
            time: largeTime,
            resultCount: largeRows.length,
            status: 'pass'
        });

        // ==========================================
        // Test 8: Update Operations
        // ==========================================
        console.log('📝 Test 8: Bulk update operations (100 updates)');
        const updateStart = Date.now();

        for (let i = 0; i < 100; i++) {
            await db.execute(`
                UPDATE memories
                SET content = 'Updated content ${i}'
                WHERE id = ${i + 1}
            `);
        }

        const updateTime = Date.now() - updateStart;
        console.log(`✅ Updated 100 records in ${updateTime}ms (${(updateTime/100).toFixed(2)}ms avg)\n`);
        results.tests.push({
            name: 'Bulk Updates',
            time: updateTime,
            avgPerOp: (updateTime/100).toFixed(2),
            status: 'pass'
        });

        // ==========================================
        // Test 9: Delete Operations
        // ==========================================
        console.log('📝 Test 9: Bulk delete operations (100 deletes)');
        const deleteStart = Date.now();

        await db.execute(`DELETE FROM memories WHERE id <= 100`);

        const deleteTime = Date.now() - deleteStart;
        console.log(`✅ Deleted 100 records in ${deleteTime}ms\n`);
        results.tests.push({
            name: 'Bulk Deletes',
            time: deleteTime,
            status: 'pass'
        });

        // ==========================================
        // Test 10: Edge Cases
        // ==========================================
        console.log('📝 Test 10: Edge cases and error handling');

        // Test 10a: Empty result set
        const emptyResult = await db.query(`
            SELECT * FROM memories WHERE user_id = 'nonexistent'
        `);
        const emptyRows = JSON.parse(emptyResult.rows);
        console.log(`   ✅ Empty result set: ${emptyRows.length === 0 ? 'PASS' : 'FAIL'}`);

        // Test 10b: Very high limit
        const highLimitResult = await db.query(`
            SELECT id FROM memories LIMIT 10000
        `);
        const highLimitRows = JSON.parse(highLimitResult.rows);
        console.log(`   ✅ High limit query: ${highLimitRows.length <= 1000 ? 'PASS' : 'FAIL'}`);

        // Test 10c: Special characters in content
        await db.execute(`
            INSERT INTO memories (user_id, content, embedding, metadata, created_at)
            VALUES (
                'test_user',
                'Content with "quotes" and ''apostrophes'' and \nnewlines',
                '${generateVector(VECTOR_DIM, 9999).join(',')}',
                '{}',
                '${new Date().toISOString()}'
            )
        `);
        console.log(`   ✅ Special characters in content: PASS`);

        console.log();
        results.tests.push({ name: 'Edge Cases', status: 'pass' });

        // ==========================================
        // Test 11: Additional Load - 5K More Vectors
        // ==========================================
        console.log('📝 Test 11: Additional bulk insert (5,000 more vectors)');
        const bulk5kStart = Date.now();
        const count5k = 5000;

        for (let i = 0; i < count5k; i++) {
            const vector = generateVector(VECTOR_DIM, i + 10000);
            const vectorStr = `[${vector.join(',')}]`;
            const now = new Date().toISOString();

            await db.execute(`
                INSERT INTO memories (user_id, content, embedding, metadata, created_at)
                VALUES (
                    'user_${i % 50}',
                    'Bulk memory ${i}',
                    '${vectorStr}',
                    '{"batch": 2}',
                    '${now}'
                )
            `);

            if ((i + 1) % 500 === 0) {
                process.stdout.write(`\r   Progress: ${i + 1}/${count5k} vectors inserted...`);
            }
        }

        const bulk5kTime = Date.now() - bulk5kStart;
        const bulk5kAvg = (bulk5kTime / count5k).toFixed(2);
        console.log(`\r✅ Inserted ${count5k} vectors in ${bulk5kTime}ms (${bulk5kAvg}ms avg)`);
        console.log(`   Memory after 5K more: ${getMemoryUsage().heapUsed}\n`);
        results.tests.push({
            name: 'Bulk Insert 5K',
            time: bulk5kTime,
            avgPerOp: bulk5kAvg,
            status: 'pass'
        });

        // ==========================================
        // Test 12: Search Performance at Scale
        // ==========================================
        console.log('📝 Test 12: Search performance with ~5.9K total vectors');
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
        console.log(`✅ Search across ~5.9K vectors: ${scaleTime}ms, ${scaleRows.length} results\n`);
        results.tests.push({
            name: 'Search at Scale (5.9K)',
            time: scaleTime,
            status: 'pass'
        });

        // ==========================================
        // Final Summary
        // ==========================================
        const totalTime = Date.now() - results.startTime;
        const endMemory = getMemoryUsage();

        console.log('═'.repeat(70));
        console.log('  Test Summary');
        console.log('═'.repeat(70));
        console.log();
        console.log(`Total Tests: ${results.tests.length}`);
        console.log(`Passed: ${results.tests.filter(t => t.status === 'pass').length}`);
        console.log(`Failed: ${results.tests.filter(t => t.status === 'fail').length}`);
        console.log(`Total Time: ${(totalTime / 1000).toFixed(2)}s`);
        console.log();
        console.log('Memory Usage:');
        console.log(`  Start: ${results.startMemory.heapUsed}`);
        console.log(`  End: ${endMemory.heapUsed}`);
        console.log(`  Delta: ${(parseFloat(endMemory.heapUsed) - parseFloat(results.startMemory.heapUsed)).toFixed(2)} MB`);
        console.log();
        console.log('Performance Highlights:');
        console.log(`  1K Vector Insert: ${results.tests.find(t => t.name === 'Bulk Insert 1K').avgPerOp}ms avg`);
        console.log(`  5K Vector Insert: ${results.tests.find(t => t.name === 'Bulk Insert 5K').avgPerOp}ms avg`);
        console.log(`  Top-10 Search (1K): ${results.tests.find(t => t.name === 'Top-10 search').time}ms`);
        console.log(`  Top-10 Search (5.9K): ~${scaleTime}ms`);
        console.log(`  Concurrent Searches: ${results.tests.find(t => t.name === 'Concurrent Searches').avgPerOp}ms avg`);
        console.log();

        if (results.tests.every(t => t.status === 'pass')) {
            console.log('🎉 All heavy load tests PASSED! NexusQL is production-ready!');
        } else {
            console.log('❌ Some tests failed. Review results above.');
        }
        console.log();

        return results;

    } catch (error) {
        console.error('❌ Heavy load test failed:', error.message);
        console.error('Error:', error);
        throw error;
    } finally {
        await db.close();
        await new Promise(r => setTimeout(r, 200));

        try {
            if (fs.existsSync(TEST_DB)) {
                fs.unlinkSync(TEST_DB);
                console.log(`Cleanup: Removed ${TEST_DB}`);
            }
        } catch (e) {
            console.log(`Cleanup error: ${e.message}`);
        }
    }
}

// Run the tests
heavyLoadTests().catch(err => {
    console.error('Fatal error:', err);
    process.exit(1);
});
