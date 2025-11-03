/**
 * NexusQL Database Client
 *
 * @example
 * ```typescript
 * const db = new NexusDB('./mydb');
 * await db.execute('CREATE TABLE users (id INT, name TEXT)');
 * const result = await db.query('SELECT * FROM users');
 * ```
 */
export declare class NexusDB {
    private dbPath;
    constructor(dbPath: string);
    /**
     * Execute a SQL query
     * @param sql - SQL query string
     * @returns Query result set
     */
    query(sql: string): Promise<QueryResult>;
    /**
     * Execute a SQL command (INSERT, UPDATE, DELETE)
     * @param sql - SQL command string
     * @returns Number of affected rows
     */
    execute(sql: string): Promise<number>;
    /**
     * Begin a transaction
     */
    beginTransaction(): Promise<Transaction>;
    /**
     * Prepare a statement for repeated execution
     * @param sql - SQL with placeholders ($1, $2, etc.)
     */
    prepare(sql: string): Promise<PreparedStatement>;
    /**
     * Close database connection
     */
    close(): Promise<void>;
}
/**
 * Query result interface
 */
export interface QueryResult {
    rows: Row[];
    rowCount: number;
    fields: Field[];
}
export interface Row {
    [key: string]: any;
}
export interface Field {
    name: string;
    type: string;
}
/**
 * Transaction management
 */
export declare class Transaction {
    private db;
    private active;
    constructor(db: NexusDB);
    query(sql: string): Promise<QueryResult>;
    commit(): Promise<void>;
    rollback(): Promise<void>;
}
/**
 * Prepared statement
 */
export declare class PreparedStatement {
    private sql;
    constructor(sql: string);
    execute(...params: any[]): Promise<QueryResult>;
    close(): Promise<void>;
}
/**
 * Connection pool for high-performance applications
 */
export declare class ConnectionPool {
    private maxConnections;
    constructor(options: PoolOptions);
    getConnection(): Promise<NexusDB>;
    releaseConnection(db: NexusDB): Promise<void>;
}
export interface PoolOptions {
    max?: number;
    min?: number;
    idleTimeoutMillis?: number;
}
export * from './types';
