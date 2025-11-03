// Type definitions for NexusQL

/**
 * Query result row
 */
export interface Row {
    [key: string]: any;
}

/**
 * Query result field definition
 */
export interface Field {
    name: string;
    type: string;
}

/**
 * Query result set
 */
export interface QueryResult {
    rows: Row[];
    rowCount: number;
    fields: Field[];
}

/**
 * Connection pool options
 */
export interface PoolOptions {
    max?: number;
    min?: number;
    idleTimeoutMillis?: number;
}

/**
 * Value types supported by NexusQL
 */
export type NexusValue = string | number | boolean | null | Uint8Array;

/**
 * Transaction isolation levels
 */
export enum IsolationLevel {
    ReadUncommitted = 'READ UNCOMMITTED',
    ReadCommitted = 'READ COMMITTED',
    RepeatableRead = 'REPEATABLE READ',
    Serializable = 'SERIALIZABLE'
}

/**
 * Connection options
 */
export interface ConnectionOptions {
    database?: string;
    isolationLevel?: IsolationLevel;
    maxRetries?: number;
}
