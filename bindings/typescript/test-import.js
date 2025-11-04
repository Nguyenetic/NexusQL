// Quick test of NexusQL native module
const { NexusDB } = require('./nexusql.win32-x64-msvc.node');

console.log('✅ NexusDB imported successfully!');
console.log('NexusDB constructor:', typeof NexusDB);

try {
    const db = new NexusDB('./test_db');
    console.log('✅ NexusDB instance created!');
    console.log('Database object:', db);
    console.log('Available methods:', Object.getOwnPropertyNames(Object.getPrototypeOf(db)));
} catch (error) {
    console.error('❌ Error creating NexusDB:', error);
}

console.log('\n🎉 NexusQL native module is working!');
