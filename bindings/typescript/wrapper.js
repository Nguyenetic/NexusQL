/**
 * User-friendly wrapper for NexusDB
 * Provides automatic parameter handling and JSON parsing
 */

const { NexusDb: NativeNexusDb } = require('./index.js');

class NexusDb {
    constructor(path, options = {}) {
        this._db = new NativeNexusDb(path);
        this._options = options;
    }

    /**
     * Execute SQL with automatic parameter handling
     * @param {string} sql - SQL query
     * @param {array} [params] - Optional parameters
     * @returns {Promise<number>} - Number of affected rows
     */
    async execute(sql, params = null) {
        if (params && params.length > 0) {
            return await this._db.executeWithParams(sql, params);
        } else {
            return await this._db.execute(sql);
        }
    }

    /**
     * Query with automatic JSON parsing
     * @param {string} sql - SQL query
     * @param {array} [params] - Optional parameters
     * @returns {Promise<object>} - {rows: Array, rowCount: number, fields: Array}
     */
    async query(sql, params = null) {
        let result;
        if (params && params.length > 0) {
            result = await this._db.queryWithParams(sql, params);
        } else {
            result = await this._db.query(sql);
        }

        // Automatically parse JSON rows
        return {
            rows: JSON.parse(result.rows),
            rowCount: result.rowCount,
            fields: result.fields
        };
    }

    /**
     * Original methods (for backwards compatibility)
     */
    async executeWithParams(sql, params) {
        return await this._db.executeWithParams(sql, params);
    }

    async queryWithParams(sql, params) {
        const result = await this._db.queryWithParams(sql, params);
        return {
            rows: JSON.parse(result.rows),
            rowCount: result.rowCount,
            fields: result.fields
        };
    }

    /**
     * Prepared statements
     */
    async prepare(sql) {
        return await this._db.prepare(sql);
    }

    /**
     * Transactions
     */
    async beginTransaction() {
        return await this._db.beginTransaction();
    }

    /**
     * Vector operations
     */
    async insertVector(table, vector, metadata) {
        return await this._db.insertVector(table, vector, metadata);
    }

    async vectorSearch(table, queryVector, limit, distanceMetric) {
        const result = await this._db.vectorSearch(table, queryVector, limit, distanceMetric);
        return JSON.parse(result);
    }

    async createVectorIndex(table, column, indexType, config) {
        return await this._db.createVectorIndex(table, column, indexType, config);
    }

    async batchInsertVectors(table, vectors, metadataArray) {
        return await this._db.batchInsertVectors(table, vectors, metadataArray);
    }

    /**
     * Close database
     */
    async close() {
        return await this._db.close();
    }
}

module.exports = { NexusDb, NativeNexusDb };
