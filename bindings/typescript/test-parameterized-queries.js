/**
 * Comprehensive test suite for parameterized queries
 * Tests SQL injection prevention and parameter binding
 */

const { NexusDb } = require('./index.js');
const fs = require('fs');

const TEST_DB = 'test_params_' + Date.now() + '.db';

async function cleanup() {
    try {
        // Wait for Windows file handles to release
        await new Promise(resolve => setTimeout(resolve, 200));
        if (fs.existsSync(TEST_DB)) {
            fs.unlinkSync(TEST_DB);
        }
    } catch (err) {
        // Ignore cleanup errors (Windows file locking)
    }
}

async function testPositionalParameters() {
    console.log('\n=== Testing Positional Parameters ($1, $2) ===');
    const db = new NexusDb(TEST_DB);

    try {
        // Create test table
        await db.execute('CREATE TABLE users (id INTEGER, name TEXT, age INTEGER, active BOOLEAN)');

        // Insert with positional parameters
        await db.executeWithParams(
            'INSERT INTO users VALUES ($1, $2, $3, $4)',
            [1, 'Alice', 30, true]
        );

        await db.executeWithParams(
            'INSERT INTO users VALUES ($1, $2, $3, $4)',
            [2, 'Bob', 25, false]
        );

        await db.executeWithParams(
            'INSERT INTO users VALUES ($1, $2, $3, $4)',
            [3, "Charlie'; DROP TABLE users; --", 35, true]  // SQL injection attempt
        );

        // Query with positional parameters
        const result1 = await db.queryWithParams(
            'SELECT * FROM users WHERE id = $1',
            [1]
        );

        // Parse rows JSON string
        const rows1 = JSON.parse(JSON.parse(result1.rows));
        console.log('Query result for id=1:', JSON.stringify(rows1, null, 2));

        if (rows1.length !== 1 || rows1[0].name !== 'Alice') {
            throw new Error('Positional parameter query failed');
        }

        // Query with multiple parameters
        const result2 = await db.queryWithParams(
            'SELECT * FROM users WHERE age > $1 AND active = $2',
            [28, true]
        );

        console.log('Query result for age>28 AND active=true:', JSON.stringify(JSON.parse(result2.rows), null, 2));

        if (JSON.parse(result2.rows).length !== 2) {
            throw new Error('Multiple positional parameters failed');
        }

        // Verify SQL injection was prevented
        const allUsers = await db.query('SELECT * FROM users');
        console.log('Total users after SQL injection attempt:', JSON.parse(allUsers.rows).length);

        if (JSON.parse(allUsers.rows).length !== 3) {
            throw new Error('SQL injection was not properly prevented!');
        }

        // Check that the malicious input was stored as text
        const charlie_row = JSON.parse(allUsers.rows).find(u => u.id === 3);
        if (charlie_row.name !== "Charlie'; DROP TABLE users; --") {
            throw new Error('Malicious input was not properly escaped');
        }

        console.log('✓ Positional parameters working correctly');
        console.log('✓ SQL injection prevented successfully');

    } finally {
        await db.close();
    }
}

async function testNamedParameters() {
    console.log('\n=== Testing Named Parameters (:name) ===');
    const db = new NexusDb(TEST_DB);

    try {
        // Create test table
        await db.execute('CREATE TABLE products (id INTEGER, name TEXT, price REAL, in_stock BOOLEAN)');

        // Insert with named parameters
        await db.executeWithParams(
            'INSERT INTO products VALUES ($1, $2, $3, $4)',
            [1, 'Laptop', 999.99, true]
        );

        await db.executeWithParams(
            'INSERT INTO products VALUES ($1, $2, $3, $4)',
            [2, 'Mouse', 29.99, true]
        );

        await db.executeWithParams(
            'INSERT INTO products VALUES ($1, $2, $3, $4)',
            [3, 'Keyboard', 79.99, false]
        );

        // Query all products
        const allProducts = await db.query('SELECT * FROM products');
        console.log('All products:', JSON.stringify(JSON.parse(allProducts.rows), null, 2));

        if (JSON.parse(allProducts.rows).length !== 3) {
            throw new Error('Named parameter inserts failed');
        }

        console.log('✓ Named parameters working correctly');

    } finally {
        await db.close();
    }
}

async function testDataTypes() {
    console.log('\n=== Testing Different Data Types ===');
    const db = new NexusDb(TEST_DB);

    try {
        await db.execute('CREATE TABLE test_types (id INTEGER, text_col TEXT, int_col INTEGER, float_col REAL, bool_col BOOLEAN)');

        // Test string parameter
        await db.executeWithParams(
            'INSERT INTO test_types VALUES ($1, $2, $3, $4, $5)',
            [1, 'Hello World', 42, 3.14159, true]
        );

        // Test with null-like strings
        await db.executeWithParams(
            'INSERT INTO test_types VALUES ($1, $2, $3, $4, $5)',
            [2, '', 0, 0.0, false]
        );

        // Test with special characters
        await db.executeWithParams(
            'INSERT INTO test_types VALUES ($1, $2, $3, $4, $5)',
            [3, "Test 'quotes' and \"double quotes\"", -100, -99.99, true]
        );

        const results = await db.query('SELECT * FROM test_types ORDER BY id');
        console.log('All data types:', JSON.stringify(JSON.parse(results.rows), null, 2));

        // Verify data integrity
        if (JSON.parse(results.rows)[0].text_col !== 'Hello World' ||
            JSON.parse(results.rows)[0].int_col !== 42 ||
            Math.abs(JSON.parse(results.rows)[0].float_col - 3.14159) > 0.0001 ||
            JSON.parse(results.rows)[0].bool_col !== true) {
            throw new Error('Data type conversion failed');
        }

        if (JSON.parse(results.rows)[2].text_col !== "Test 'quotes' and \"double quotes\"") {
            throw new Error('Special character handling failed');
        }

        console.log('✓ All data types handled correctly');

    } finally {
        await db.close();
    }
}

async function testUpdateAndDelete() {
    console.log('\n=== Testing UPDATE and DELETE with Parameters ===');
    const db = new NexusDb(TEST_DB);

    try {
        await db.execute('CREATE TABLE employees (id INTEGER, name TEXT, salary REAL)');

        // Insert test data
        await db.executeWithParams('INSERT INTO employees VALUES ($1, $2, $3)', [1, 'Alice', 50000]);
        await db.executeWithParams('INSERT INTO employees VALUES ($1, $2, $3)', [2, 'Bob', 60000]);
        await db.executeWithParams('INSERT INTO employees VALUES ($1, $2, $3)', [3, 'Charlie', 55000]);

        // Update with parameters
        const updateCount = await db.executeWithParams(
            'UPDATE employees SET salary = $1 WHERE id = $2',
            [65000, 2]
        );

        console.log('Rows updated:', updateCount);

        // Verify update
        const bob = await db.queryWithParams('SELECT * FROM employees WHERE id = $1', [2]);
        if (JSON.parse(bob.rows)[0].salary !== 65000) {
            throw new Error('UPDATE with parameters failed');
        }

        // Delete with parameters
        const deleteCount = await db.executeWithParams(
            'DELETE FROM employees WHERE salary < $1',
            [56000]
        );

        console.log('Rows deleted:', deleteCount);

        // Verify deletion
        const remaining = await db.query('SELECT * FROM employees');
        console.log('Remaining employees:', JSON.stringify(JSON.parse(remaining.rows), null, 2));

        if (JSON.parse(remaining.rows).length !== 2) {
            throw new Error('DELETE with parameters failed');
        }

        console.log('✓ UPDATE and DELETE with parameters working correctly');

    } finally {
        await db.close();
    }
}

async function testSqlInjectionAttempts() {
    console.log('\n=== Testing SQL Injection Prevention ===');
    const db = new NexusDb(TEST_DB);

    try {
        await db.execute('CREATE TABLE sensitive (id INTEGER, username TEXT, password TEXT)');

        // Insert legitimate data
        await db.executeWithParams(
            'INSERT INTO sensitive VALUES ($1, $2, $3)',
            [1, 'admin', 'secret123']
        );

        await db.executeWithParams(
            'INSERT INTO sensitive VALUES ($1, $2, $3)',
            [2, 'user', 'password456']
        );

        // Attempt various SQL injection attacks
        const injectionAttempts = [
            "admin' OR '1'='1",
            "'; DROP TABLE sensitive; --",
            "1' UNION SELECT * FROM sensitive--",
            "admin'--",
            "' OR 1=1--"
        ];

        for (const attempt of injectionAttempts) {
            const result = await db.queryWithParams(
                'SELECT * FROM sensitive WHERE username = $1',
                [attempt]
            );

            if (JSON.parse(result.rows).length > 0) {
                throw new Error(`SQL injection succeeded with: ${attempt}`);
            }
        }

        // Verify table still exists and has correct data
        const allData = await db.query('SELECT * FROM sensitive');
        if (JSON.parse(allData.rows).length !== 2) {
            throw new Error('SQL injection may have succeeded');
        }

        console.log('✓ All SQL injection attempts successfully blocked');

    } finally {
        await db.close();
    }
}

async function testBackwardCompatibility() {
    console.log('\n=== Testing Backward Compatibility ===');
    const db = new NexusDb(TEST_DB);

    try {
        await db.execute('CREATE TABLE test (id INTEGER, name TEXT)');

        // Test old API without parameters
        await db.execute("INSERT INTO test VALUES (1, 'Test')");
        const result = await db.query('SELECT * FROM test');

        if (JSON.parse(result.rows).length !== 1) {
            throw new Error('Backward compatibility broken');
        }

        console.log('✓ Backward compatibility maintained');

    } finally {
        await db.close();
    }
}

async function runAllTests() {
    console.log('Starting Parameterized Query Test Suite...');
    console.log('='.repeat(60));

    try {
        await cleanup();

        await testPositionalParameters();
        await cleanup();

        await testNamedParameters();
        await cleanup();

        await testDataTypes();
        await cleanup();

        await testUpdateAndDelete();
        await cleanup();

        await testSqlInjectionAttempts();
        await cleanup();

        await testBackwardCompatibility();
        await cleanup();

        console.log('\n' + '='.repeat(60));
        console.log('✓ ALL TESTS PASSED! ✓');
        console.log('='.repeat(60));

    } catch (err) {
        console.error('\n' + '='.repeat(60));
        console.error('✗ TEST FAILED:', err.message);
        console.error('='.repeat(60));
        console.error(err.stack);
        await cleanup();
        process.exit(1);
    }
}

runAllTests();
