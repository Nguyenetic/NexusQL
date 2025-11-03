"use strict";
// Type definitions for NexusQL
Object.defineProperty(exports, "__esModule", { value: true });
exports.IsolationLevel = void 0;
/**
 * Transaction isolation levels
 */
var IsolationLevel;
(function (IsolationLevel) {
    IsolationLevel["ReadUncommitted"] = "READ UNCOMMITTED";
    IsolationLevel["ReadCommitted"] = "READ COMMITTED";
    IsolationLevel["RepeatableRead"] = "REPEATABLE READ";
    IsolationLevel["Serializable"] = "SERIALIZABLE";
})(IsolationLevel || (exports.IsolationLevel = IsolationLevel = {}));
