import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import HomepageFeatures from '@site/src/components/HomepageFeatures';
import Heading from '@theme/Heading';

import styles from './index.module.css';

function HomepageHeader() {
  const {siteConfig} = useDocusaurusContext();
  return (
    <header className={clsx('hero hero--primary', styles.heroBanner)}>
      <div className="container">
        <Heading as="h1" className="hero__title">
          {siteConfig.title}
        </Heading>
        <p className="hero__subtitle">{siteConfig.tagline}</p>
        <div className={styles.buttons}>
          <Link
            className="button button--secondary button--lg"
            to="/docs/guides/getting-started">
            Get Started - 5 min ⏱️
          </Link>
          <Link
            className="button button--outline button--secondary button--lg margin-left--md"
            to="/docs/api/typescript/README">
            API Reference 📚
          </Link>
        </div>
        <div className={styles.badges}>
          <img src="https://badge.fury.io/js/%40nguyenetic%2Fnexusql.svg" alt="npm version" />
          <img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT" />
          <img src="https://img.shields.io/badge/node-%3E%3D16.0.0-brightgreen.svg" alt="Node.js" />
        </div>
      </div>
    </header>
  );
}

export default function Home(): JSX.Element {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout
      title={`${siteConfig.title} - AI-Native Database`}
      description="AI-Native, PostgreSQL-compatible database with vector search, brain-inspired computing, and parameterized queries">
      <HomepageHeader />
      <main>
        <HomepageFeatures />

        <section className={styles.codeSection}>
          <div className="container">
            <div className="row">
              <div className="col col--12">
                <Heading as="h2" className="text--center margin-bottom--lg">
                  Quick Example
                </Heading>
              </div>
            </div>
            <div className="row">
              <div className="col col--6">
                <h3>Installation</h3>
                <pre><code>npm install @nguyenetic/nexusql</code></pre>
              </div>
              <div className="col col--6">
                <h3>Basic Usage</h3>
                <pre><code>{`const { NexusDb } = require('@nguyenetic/nexusql');

const db = new NexusDb('./myapp.db');

// Parameterized query (SQL injection safe)
await db.execute(
  'INSERT INTO users VALUES ($1, $2)',
  ['Alice', 'alice@example.com']
);

const result = await db.query(
  'SELECT * FROM users WHERE name = $1',
  ['Alice']
);

await db.close();`}</code></pre>
              </div>
            </div>
          </div>
        </section>

        <section className={styles.featureHighlight}>
          <div className="container">
            <div className="row">
              <div className="col col--12 text--center">
                <Heading as="h2" className="margin-bottom--lg">
                  Why NexusQL?
                </Heading>
              </div>
            </div>
            <div className="row">
              <div className="col col--4 text--center padding--md">
                <h3>🧠 Brain-Inspired AI</h3>
                <p>
                  Hyperdimensional Computing (HDC) and ColBERT for one-shot learning
                  and 100× energy efficiency
                </p>
              </div>
              <div className="col col--4 text--center padding--md">
                <h3>⚡ High Performance</h3>
                <p>
                  1578× HNSW index speedup, 49× cache speedup, 10-100× batch operations
                </p>
              </div>
              <div className="col col--4 text--center padding--md">
                <h3>🔒 Secure by Default</h3>
                <p>
                  Parameterized queries prevent SQL injection. RBAC and Row-Level Security included
                </p>
              </div>
            </div>
            <div className="row margin-top--lg">
              <div className="col col--4 text--center padding--md">
                <h3>🔍 Vector Search</h3>
                <p>
                  Built-in HNSW index for semantic search with 99%+ recall at O(log N) complexity
                </p>
              </div>
              <div className="col col--4 text--center padding--md">
                <h3>💾 ACID Compliant</h3>
                <p>
                  Full MVCC transactions with WAL and crash recovery. PostgreSQL-compatible
                </p>
              </div>
              <div className="col col--4 text--center padding--md">
                <h3>🚀 Easy to Use</h3>
                <p>
                  Modern async/await API, TypeScript support, and comprehensive documentation
                </p>
              </div>
            </div>
          </div>
        </section>
      </main>
    </Layout>
  );
}
