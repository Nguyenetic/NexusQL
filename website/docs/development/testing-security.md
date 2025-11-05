# Testing & Security Best Practices - Comprehensive Citation Guide

**Created:** 2025-01-04
**Purpose:** Research-backed references for building secure, well-tested database systems
**Total Sources:** 37+ cited academic papers, industry standards, and documentation

---

## Table of Contents

1. [Database Testing - Academic Research](#database-testing---academic-research)
2. [Vector Database Testing & Validation](#vector-database-testing--validation)
3. [SQLite Testing & Performance](#sqlite-testing--performance)
4. [Test Framework & Isolation Patterns](#test-framework--isolation-patterns)
5. [Windows File Handling & Cleanup](#windows-file-handling--cleanup)
6. [Security - SQL Injection Prevention](#security---sql-injection-prevention)
7. [Knowledge Graph Validation](#knowledge-graph-validation)
8. [Quick Reference - Key Takeaways](#quick-reference---key-takeaways)

---

## Database Testing - Academic Research

### Recent Papers (2024-2025)

#### 1. **Enhanced Differential Testing in Emerging Database Systems**
- **Published:** January 2025 (arXiv)
- **Key Finding:** Found 57 previously unknown bugs (17 logic bugs, 40 internal errors) using SQLxDiff approach
- **Tested Systems:** QuestDB, TDEngine, RisingWave, CrateDB
- **Link:** https://arxiv.org/html/2501.01236v1
- **Why It Matters:** Demonstrates cutting-edge approaches to finding logic bugs in database systems

#### 2. **Testing Database Engines via Query Plan Guidance (QPG)**
- **Published:** December 2023 (arXiv)
- **Key Finding:** Found 53 unique bugs by steering testing toward diverse query plans
- **Implemented In:** SQLancer framework
- **Link:** https://arxiv.org/html/2312.17510v1
- **Why It Matters:** Shows how query plan diversity improves test coverage

#### 3. **Testing Database Systems with LLM Synthesized Fragments (ShQveL)**
- **Published:** 2024 (arXiv)
- **Key Finding:** Found 55 bugs (50 fixed, 39 logic bugs) using LLM-generated test cases
- **Innovation:** Combines machine learning with traditional fuzzing
- **Link:** https://arxiv.org/html/2505.02012
- **Why It Matters:** Demonstrates future of AI-assisted database testing

#### 4. **SQLaser: Detecting DBMS Logic Bugs with Clause-Guided Fuzzing**
- **Published:** July 2024 (arXiv)
- **Key Finding:** 60% faster bug detection, identified 35 logic bug patterns
- **Tested On:** SQLite, MySQL, PostgreSQL, TiDB
- **Link:** https://arxiv.org/html/2407.04294v1
- **Why It Matters:** Practical fuzzing approach that significantly accelerates bug discovery

#### 5. **LLM-based Dynamic Differential Testing for Database Connectors**
- **Published:** 2025 (arXiv)
- **Innovation:** Reinforcement learning-guided test case generation
- **Link:** https://arxiv.org/abs/2506.11870
- **Why It Matters:** Shows how RL can optimize test case generation

#### 6. **A Comprehensive Survey on Database Management System Fuzzing**
- **Published:** November 2023 (arXiv)
- **Scope:** Taxonomy and experimental comparison of DBMS fuzzing techniques
- **Link:** https://arxiv.org/abs/2311.06728
- **Why It Matters:** Comprehensive overview of the fuzzing landscape

---

### VLDB Conference Papers (2024-2025)

#### 7. **IsoVista: Black-Box Checking Database Isolation Guarantees**
- **Conference:** VLDB 2024
- **Key Finding:** First black-box isolation checking system with no false positives
- **Link:** https://dl.acm.org/doi/10.14778/3685800.3685866
- **Why It Matters:** Critical for verifying transactional isolation correctness

#### 8. **Fucci: Database Transaction Fuzzing via Random Conflict**
- **Conference:** VLDB 2025
- **Key Finding:** 3.1× more bugs than DT2, 1.7× more than Troc after 24 hours
- **Link:** https://www.vldb.org/pvldb/vol18/p1879-li.pdf
- **Why It Matters:** Most effective transaction fuzzing tool currently available

#### 9. **Detecting Metadata-Related Logic Bugs (Radar)**
- **Conference:** VLDB 2024
- **Approach:** Raw database construction for testing metadata optimizations
- **Link:** https://dl.acm.org/doi/10.14778/3659437.3659445
- **Why It Matters:** Addresses a specific class of bugs often missed by other approaches

#### 10. **DLBD: Database Logic Bug Detection System**
- **Conference:** VLDB 2024
- **Features:** Automatic schema/query generation with ground-truth retrieval
- **Link:** https://dl.acm.org/doi/abs/10.14778/3611540.3611584
- **Why It Matters:** Holistic approach to logic bug detection

#### 11. **SQLStorm: Taking Database Benchmarking into the LLM Era**
- **Conference:** VLDB 2024
- **Key Finding:** Fixed 90% of crashes within one month using LLM-generated queries
- **Link:** https://www.vldb.org/pvldb/vol18/p4144-schmidt.pdf
- **Why It Matters:** Demonstrates practical value of LLMs in production testing

---

### ACM SIGMOD Papers (2024)

#### 12. **Spatter: Spatial DBMS Testing**
- **Conference:** SIGMOD 2024 (DBTest Workshop)
- **Key Finding:** Found 14 logic bugs via Affine Equivalent Inputs (AEI)
- **Tested On:** PostGIS, DuckDB Spatial, MySQL, SQL Server
- **Why It Matters:** Specialized approach for spatial database testing

#### 13. **QTRAN: Extending Metamorphic-Oracle Based Logical Bug Detection**
- **Conference:** ACM SIGMOD 2024
- **Innovation:** Multiple-DBMS dialect support for metamorphic testing
- **Link:** https://dl.acm.org/doi/10.1145/3728908
- **Why It Matters:** Makes metamorphic testing portable across different databases

#### 14. **THANOS: Storage Engine Rotation Based Differential Testing**
- **Conference:** IEEE/ACM ICSE 2025
- **Award:** Distinguished Paper Award
- **Why It Matters:** Novel approach using storage engine rotation for differential testing

#### 15. **DepState: Detecting Synchronization Failure Bugs in Distributed DBMS**
- **Published:** ACM 2024
- **Link:** https://dl.acm.org/doi/10.1145/3728965
- **Why It Matters:** Critical for distributed database testing

---

### Test Oracle & Correctness Verification

#### 16. **Metamorphic Testing with SQLancer**
- **Note:** SQLancer uses NoREC and TLP as state-of-the-art test oracles
- **Key Insight:** Requires ~3K lines of Java code per supported DBMS
- **Why It Matters:** Industry-standard tool for finding logic bugs

#### 17. **DBTest Workshop**
- **Purpose:** Bring researchers and practitioners together on testing challenges
- **Link:** https://dbtest-workshop.github.io/
- **Why It Matters:** Community hub for database testing research

---

## Vector Database Testing & Validation

### Performance & Benchmarking

#### 18. **Vector Search Performance: Speed & Scalability Benchmarks**
- **Publisher:** NewTuple (2024)
- **Methodology:** 100 queries with 100 concurrent requests
- **Link:** https://www.newtuple.com/post/speed-and-scalability-in-vector-search
- **Why It Matters:** Standard benchmarking methodology for vector search

#### 19. **Open-Source Vector Database Benchmarking**
- **Publisher:** BlueTeam AI (2024)
- **Scope:** Comparative analysis of open-source vector databases
- **Link:** https://blueteam.ai/blog/vector-benchmarking/
- **Why It Matters:** Independent benchmarks for vector database selection

#### 20. **SQLite vs. Chroma: Comparative Analysis for Vector Embeddings**
- **Published:** 2024 (DEV Community)
- **Key Finding:** ChromaDB excels at single requests but degrades under concurrency
- **Link:** https://dev.to/stephenc222/sqlite-vs-chroma-a-comparative-analysis-for-managing-vector-embeddings-4i76
- **Why It Matters:** Real-world performance comparison

---

### Embedding Evaluation Methods

#### 21. **How to Evaluate Embedding Models for Specific Tasks**
- **Publisher:** Zilliz
- **Methods:** Intrinsic (word similarity, analogy) vs Extrinsic (task-specific)
- **Link:** https://zilliz.com/ai-faq/how-do-i-evaluate-embedding-models-for-specific-downstream-tasks
- **Why It Matters:** Framework for validating embedding quality

#### 22. **Intrinsic vs Extrinsic Evaluation of Embeddings**
- **Publisher:** Zilliz
- **Link:** https://zilliz.com/ai-faq/how-do-i-perform-intrinsic-versus-extrinsic-evaluation-of-embeddings
- **Why It Matters:** Dual evaluation approach ensures comprehensive testing

#### 23. **Embedding Distance Evaluation**
- **Publisher:** LangChain Documentation
- **Metrics:** Cosine, Euclidean, Manhattan, Chebyshev, Hamming
- **Link:** https://python.langchain.com/v0.1/docs/guides/productionization/evaluation/string/embedding_distance/
- **Why It Matters:** Standard metrics for semantic similarity validation

#### 24. **Vector Similarity Explained**
- **Publisher:** Pinecone
- **Key Insight:** Match similarity metric to the one used to train your embedding model
- **Link:** https://www.pinecone.io/learn/vector-similarity/
- **Why It Matters:** Ensures accurate search results

#### 25. **Choose Vector Distance Functions**
- **Publisher:** Google Cloud Spanner Documentation
- **Guidance:** When vectors are normalized, Euclidean, cosine, and dot product are proportional
- **Link:** https://cloud.google.com/spanner/docs/choose-vector-distance-function
- **Why It Matters:** Understanding when different metrics produce equivalent results

---

## SQLite Testing & Performance

### Official SQLite Documentation

#### 26. **How SQLite Is Tested**
- **Publisher:** Official SQLite Documentation
- **Coverage:** 100% branch coverage, millions of test cases
- **Link:** https://www.sqlite.org/testing.html
- **Why It Matters:** Gold standard for database testing practices

#### 27. **Isolation In SQLite**
- **Publisher:** Official SQLite Documentation
- **Key Concept:** WAL mode provides "snapshot isolation"
- **Link:** https://sqlite.org/isolation.html
- **Why It Matters:** Understanding isolation semantics for correct testing

#### 28. **Write-Ahead Logging (WAL)**
- **Publisher:** Official SQLite Documentation
- **Key Benefit:** Readers don't block writers, writers don't block readers
- **Link:** https://sqlite.org/wal.html
- **Why It Matters:** Performance optimization for concurrent access

#### 29. **Standard File Control Opcodes**
- **Publisher:** Official SQLite Documentation
- **Windows Defaults:** 10 retries, 25ms delay between attempts
- **Link:** https://sqlite.org/c3ref/c_fcntl_busyhandler.html
- **Why It Matters:** Understanding SQLite's built-in retry mechanisms

#### 30. **Temporary Files Used By SQLite**
- **Publisher:** Official SQLite Documentation
- **Key Info:** WAL and SHM files are in same directory as database file
- **Link:** https://sqlite.org/tempfiles.html
- **Why It Matters:** Complete cleanup requires handling auxiliary files

---

### Performance Tuning

#### 31. **SQLite Performance Tuning**
- **Author:** phiresky
- **Scope:** Scaling to many concurrent readers, maintaining 100k SELECTs/sec
- **Link:** https://phiresky.github.io/blog/2020/sqlite-performance-tuning/
- **Why It Matters:** Practical guide to production-level SQLite performance

#### 32. **SQLite Pragma Cheatsheet**
- **Author:** Clément Joly
- **Focus:** Performance and consistency pragmas
- **Link:** https://cj.rs/blog/sqlite-pragma-cheatsheet-for-performance-and-consistency/
- **Why It Matters:** Quick reference for optimization

#### 33. **SQLite Concurrency: Thread Safety, WAL Mode, and Beyond**
- **Published:** 2024 (iifx.dev)
- **Link:** https://iifx.dev/en/articles/17373144
- **Why It Matters:** Understanding concurrent access patterns

---

## Test Framework & Isolation Patterns

### Node.js Testing Best Practices (2024)

#### 34. **Best Testing Practices in Node.js**
- **Publisher:** AppSignal Blog (October 2024)
- **Link:** https://blog.appsignal.com/2024/10/16/best-testing-practices-in-nodejs.html
- **Why It Matters:** Current best practices from industry experts

#### 35. **Integration Testing with NodeJs, Jest, Knex and SQLite In-Memory**
- **Published:** 2024 (DEV Community)
- **Key Recommendation:** Use in-memory databases for fast, isolated tests
- **Link:** https://dev.to/rukykf/integration-testing-with-nodejs-jest-knex-and-sqlite-in-memory-databases-2ila
- **Why It Matters:** Practical guide to in-memory testing

#### 36. **Test Isolation with Jest in TypeScript**
- **Publisher:** Kainos (2024)
- **Key Tool:** jest.resetModules() for module isolation
- **Link:** https://www.kainos.com/insights/blogs/test-isolation-with-jest-in-typescript
- **Why It Matters:** Ensuring true test isolation

---

### Temporary File Management

#### 37. **How to Manage Temporary Files with Pytest tmp_path**
- **Publisher:** Pytest with Eric (2024)
- **Key Feature:** Automatic cleanup of temporary directories
- **Link:** https://pytest-with-eric.com/pytest-best-practices/pytest-tmp-path/
- **Why It Matters:** Pattern applicable to Node.js testing

#### 38. **Python: Create Temporary Files and Directories in unittest**
- **Author:** Adam Johnson (December 2024)
- **Innovation:** enterContext() method for cleaner test code
- **Link:** https://adamj.eu/tech/2024/12/30/python-temporary-files-directories-unittest/
- **Why It Matters:** Cross-language patterns for test fixtures

#### 39. **tmp - npm Package**
- **Features:** Automatic cleanup on process exit, cross-platform support
- **Link:** https://www.npmjs.com/package/tmp
- **Why It Matters:** Production-ready solution for Node.js temporary files

---

### Test Framework Comparison (2024)

#### 40. **JavaScript Unit Testing Frameworks in 2024: A Comparison**
- **Publisher:** Raygun Blog
- **Frameworks:** Jest, Mocha, Jasmine, Vitest
- **Link:** https://raygun.com/blog/javascript-unit-testing-frameworks/
- **Why It Matters:** Informed framework selection

#### 41. **Jest vs Mocha: What's the Difference?**
- **Publisher:** Pieces.app
- **Key Finding:** Jest for ease-of-use, Mocha for flexibility
- **Link:** https://pieces.app/blog/whats-the-difference-between-jest-and-mocha
- **Why It Matters:** Understanding tradeoffs between frameworks

---

## Windows File Handling & Cleanup

### File Locking Solutions

#### 42. **Understanding Node.js File Locking**
- **Publisher:** LogRocket Blog (2024)
- **Key Insight:** Different operating systems handle file locking differently
- **Link:** https://blog.logrocket.com/understanding-node-js-file-locking/
- **Why It Matters:** Understanding cross-platform differences

#### 43. **proper-lockfile - npm Package**
- **Features:** Inter-process lockfile utility with exponential backoff
- **Strategy:** mkdir-based locking (works on network filesystems)
- **Link:** https://www.npmjs.com/package/proper-lockfile
- **Why It Matters:** Robust cross-platform file locking

#### 44. **rimraf - npm Package**
- **Windows Defaults:** 1.2 backoff rate, 10 retries, 200ms max backoff
- **Special Handling:** Works around EPERM errors on Windows
- **Link:** https://www.npmjs.com/package/rimraf
- **Why It Matters:** Proven solution for Windows file deletion

#### 45. **SQLite Windows VFS Retry Pattern**
- **Source:** SQLite Documentation
- **Defaults:** 10 retries, 25ms delay, 1.2× exponential backoff
- **Reason:** Robustness against antivirus programs
- **Link:** https://sqlite.org/c3ref/c_fcntl_busyhandler.html
- **Why It Matters:** SQLite's own proven approach to Windows issues

---

### Graceful Shutdown Patterns

#### 46. **Graceful Shutdown in NodeJS**
- **Author:** nairihar (Medium, 2024)
- **Key Steps:** Listen for SIGTERM/SIGINT, close connections, call process.exit()
- **Link:** https://nairihar.medium.com/graceful-shutdown-in-nodejs-2f8f59d1c357
- **Why It Matters:** Proper resource cleanup on shutdown

#### 47. **NodeJS Graceful Shutdown: A Beginner's Guide**
- **Publisher:** DEV Community (2024)
- **Link:** https://dev.to/yusadolat/nodejs-graceful-shutdown-a-beginners-guide-40b6
- **Why It Matters:** Step-by-step implementation guide

#### 48. **Implement Retries with Exponential Backoff**
- **Publisher:** Microsoft .NET Documentation
- **Industry Standard:** Exponential backoff with jitter
- **Link:** https://learn.microsoft.com/en-us/dotnet/architecture/microservices/implement-resilient-applications/implement-retries-exponential-backoff
- **Why It Matters:** Microsoft's official guidance on retry patterns

#### 49. **Timeouts, Retries and Backoff with Jitter**
- **Publisher:** AWS Builders Library
- **Link:** https://aws.amazon.com/builders-library/timeouts-retries-and-backoff-with-jitter/
- **Why It Matters:** AWS best practices for distributed systems

---

## Security - SQL Injection Prevention

### Node.js SQL Security

#### 50. **Node.js SQL Injection Guide: Examples and Prevention**
- **Publisher:** StackHawk
- **Primary Defense:** Parameterized queries
- **Link:** https://www.stackhawk.com/blog/node-js-sql-injection-guide-examples-and-prevention/
- **Why It Matters:** Comprehensive security guide for Node.js

#### 51. **How to Prevent SQL Injection Attacks in Node.js**
- **Publisher:** PlanetScale Blog
- **Key Tools:** Prisma, Drizzle ORMs with built-in protection
- **Link:** https://planetscale.com/blog/how-to-prevent-sql-injection-attacks-in-node-js
- **Why It Matters:** ORM-based security approaches

#### 52. **Preventing SQL Injection Attacks in Node.js**
- **Publisher:** Snyk Blog
- **Link:** https://snyk.io/blog/preventing-sql-injection-attacks-node-js/
- **Why It Matters:** Security-focused perspective

#### 53. **Does JSON.stringify Protect Against SQL Injection?**
- **Source:** Stack Overflow
- **Answer:** NO - Comments like `--` or `#` can still break queries
- **Link:** https://stackoverflow.com/questions/30170363/does-json-stringify-a-string-protect-against-mysql-injection
- **Why It Matters:** Common misconception debunked

#### 54. **Top Node.js Security Threats: Prevent SQL Injection & XSS**
- **Publisher:** JavaScript Conference Blog
- **Link:** https://javascript-conference.com/blog/node-js-security-vulnerabilities-sql-xss-prevention/
- **Why It Matters:** Broader security context

---

## Knowledge Graph Validation

### Testing & Validation Methods

#### 55. **Knowledge Graph Validation**
- **Publisher:** WordLift Blog
- **Key Concept:** Evaluate whether statements correspond to real world
- **Link:** https://wordlift.io/blog/en/knowledge-graph-validation/
- **Why It Matters:** Framework for KG correctness validation

#### 56. **Efficient Knowledge Graph Validation via Cross-Graph Representation**
- **Publisher:** NSF (National Science Foundation)
- **Key Metric:** 75% f-measure validation accuracy
- **Link:** https://par.nsf.gov/servlets/purl/10209161
- **Why It Matters:** Academic approach to KG validation

#### 57. **Automating Knowledge Graph Creation with LLMs**
- **Author:** Anna Alexandra Grigoryan (Medium)
- **Innovation:** LLM-based entity/relationship extraction
- **Link:** https://thegrigorian.medium.com/automating-knowledge-graph-creation-and-validation-with-large-language-models-9bfedf5d5904
- **Why It Matters:** Future of automated KG validation

#### 58. **Towards Knowledge Graphs Validation Through Weighted Knowledge Sources**
- **Publisher:** Springer
- **Method:** Confidence scoring via external knowledge source matching
- **Link:** https://link.springer.com/chapter/10.1007/978-3-030-91305-2_4
- **Why It Matters:** Weighted validation approach

---

## Quick Reference - Key Takeaways

### Testing Best Practices

1. **Use In-Memory Databases** (`:memory:`)
   - Industry standard 2024 (Sources: #35, #36, #34)
   - Eliminates file cleanup issues
   - Faster tests with perfect isolation

2. **Implement Proper Test Oracles**
   - Metamorphic testing (Sources: #1, #13, #16)
   - Differential testing (Sources: #1, #2, #14)
   - Query plan guidance (Sources: #2)

3. **Fuzzing Approaches**
   - Clause-guided fuzzing 60% faster (Source: #4)
   - LLM-based generation (Sources: #3, #11)
   - Transaction conflict testing (Source: #8)

---

### Vector Database Testing

1. **Similarity Metrics**
   - Match training metric (Source: #24)
   - Cosine for all-MiniLM-L6-v2 (Source: #24)
   - Normalized vectors make metrics equivalent (Source: #25)

2. **Validation Methods**
   - Intrinsic: word similarity, analogy tests (Source: #21)
   - Extrinsic: task-specific metrics (recall@k) (Source: #21, #23)
   - Cross-validation for generalization (Source: #21)

3. **Benchmarking**
   - 100 queries × 100 concurrent requests (Source: #18)
   - Test realistic dataset sizes (500-1000+ records)
   - Measure query latency, precision, recall (Source: #18, #19)

---

### SQLite Best Practices

1. **WAL Mode** (Sources: #27, #28, #33)
   - Enable with `PRAGMA journal_mode = WAL`
   - Readers don't block writers
   - Requires checkpoint management

2. **Optimization** (Sources: #31, #32)
   - Run `PRAGMA optimize` before close
   - Use `PRAGMA analysis_limit=400`
   - Checkpoint with `wal_checkpoint(TRUNCATE)`

3. **Testing** (Source: #26)
   - SQLite uses 100% branch coverage
   - Millions of test cases
   - Both in-memory and file-based tests

---

### Windows File Handling

1. **Exponential Backoff** (Sources: #44, #45, #48)
   - 10 retries (SQLite/rimraf standard)
   - 25ms base delay
   - 1.2× backoff rate
   - 200ms max backoff

2. **Proper Shutdown** (Sources: #46, #47)
   - Run `PRAGMA optimize`
   - Checkpoint WAL
   - Close connection
   - Wait 100ms for handle release

3. **Better Solutions** (Source: #39)
   - Use `tmp` npm package
   - Automatic cleanup on exit
   - Cross-platform support

---

### Security

1. **SQL Injection Prevention** (Sources: #50, #51, #52)
   - **ALWAYS** use parameterized queries
   - **NEVER** concatenate user input
   - JSON.stringify is NOT protection (Source: #53)

2. **ORMs with Built-in Protection** (Source: #51)
   - Prisma
   - Drizzle
   - TypeORM

3. **Input Validation** (Source: #50)
   - Type validation
   - Sanitization
   - Transformation

---

### Test Framework Selection (2024)

**Jest** (Sources: #40, #41)
- Zero config
- Built-in mocking
- Parallel execution
- Best for React/TypeScript

**Mocha** (Sources: #40, #41)
- Maximum flexibility
- Choose your own assertion library
- Sequential execution for easier debugging
- Best for custom setups

---

## Implementation Checklist

### For Your New Project

- [ ] Use `:memory:` for unit tests
- [ ] Implement proper test oracles (metamorphic/differential)
- [ ] Set up fuzzing pipeline (consider SQLaser or LLM-based)
- [ ] Add vector similarity validation (cosine for all-MiniLM-L6-v2)
- [ ] Configure WAL mode with proper checkpointing
- [ ] Use parameterized queries exclusively
- [ ] Set up graceful shutdown (PRAGMA optimize, cleanup)
- [ ] Implement exponential backoff for file operations
- [ ] Add knowledge graph validation (75% f-measure target)
- [ ] Configure Jest/Mocha with proper isolation

### Testing Coverage Goals

- [ ] 100% branch coverage (SQLite standard)
- [ ] Metamorphic test cases for all query types
- [ ] Isolation level validation (IsoVista approach)
- [ ] Concurrent access testing (100 queries × 100 requests)
- [ ] Vector search benchmarks (1ms target for small datasets)
- [ ] Batch insert performance tests
- [ ] Cross-platform testing (Windows, macOS, Linux)

---

## Recommended Reading Order

**If you're new to database testing:**
1. Start with #26 (How SQLite Is Tested)
2. Read #34 (Best Testing Practices in Node.js)
3. Review #35 (In-Memory Database Testing)
4. Study #50 (SQL Injection Prevention)

**If you're building a vector database:**
1. Start with #18 (Vector Search Benchmarks)
2. Read #21 (Embedding Evaluation)
3. Study #24 (Vector Similarity Explained)
4. Review #19 (Vector Database Benchmarking)

**If you're implementing fuzzing:**
1. Start with #4 (SQLaser)
2. Read #6 (Comprehensive Survey)
3. Study #8 (Fucci Transaction Fuzzing)
4. Review #1 (Enhanced Differential Testing)

**If you're fixing Windows issues:**
1. Start with #42 (Understanding File Locking)
2. Read #44 (rimraf approach)
3. Study #46 (Graceful Shutdown)
4. Implement #39 (tmp package)

---

## Tools & Libraries Referenced

### Testing Frameworks
- **Jest** - https://jestjs.io/
- **Mocha** - https://mochajs.org/
- **SQLancer** - Metamorphic testing tool for databases

### File Handling
- **tmp** - https://www.npmjs.com/package/tmp
- **rimraf** - https://www.npmjs.com/package/rimraf
- **proper-lockfile** - https://www.npmjs.com/package/proper-lockfile

### Database Tools
- **better-sqlite3** - https://github.com/WiseLibs/better-sqlite3
- **Prisma** - https://www.prisma.io/
- **Drizzle** - https://orm.drizzle.team/

### Vector Search
- **all-MiniLM-L6-v2** - Sentence transformer model (384 dimensions)
- **ChromaDB** - Vector database
- **Pinecone** - Managed vector database

---

## Updates & Maintenance

**This document should be updated when:**
- New VLDB/SIGMOD papers are published
- Better testing methodologies emerge
- Security vulnerabilities are discovered
- Performance benchmarks change significantly

**Last Research Date:** January 4, 2025
**Next Recommended Update:** After VLDB 2025 (September), SIGMOD 2026 (June)

---

## Contributing

If you find errors or have additional citations to add:
1. Verify the source is from 2024 or later (or seminal work)
2. Include full citation with link
3. Explain "Why It Matters"
4. Add to appropriate category

---

**End of Document**

Total Citations: 58 sources across 7 categories
