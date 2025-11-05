import clsx from 'clsx';
import Heading from '@theme/Heading';
import styles from './styles.module.css';

type FeatureItem = {
  title: string;
  emoji: string;
  description: JSX.Element;
};

const FeatureList: FeatureItem[] = [
  {
    title: 'AI-Native Database',
    emoji: '🧠',
    description: (
      <>
        Built with Hyperdimensional Computing (HDC) and ColBERT for brain-inspired
        AI. One-shot learning with 100× energy efficiency compared to deep learning.
      </>
    ),
  },
  {
    title: 'Vector Search Built-In',
    emoji: '🔍',
    description: (
      <>
        HNSW index provides 1578× speedup for semantic search with 99%+ recall.
        Perfect for RAG, embeddings, and AI applications.
      </>
    ),
  },
  {
    title: 'SQL Injection Prevention',
    emoji: '🔒',
    description: (
      <>
        Parameterized queries with $1, $2 placeholders prevent SQL injection attacks.
        Built-in security with RBAC and Row-Level Security (RLS).
      </>
    ),
  },
  {
    title: 'High Performance',
    emoji: '⚡',
    description: (
      <>
        49× query cache speedup, 10-100× batch operations, prepared statements
        for bulk inserts. Optimized for production workloads.
      </>
    ),
  },
  {
    title: 'PostgreSQL Compatible',
    emoji: '🗄️',
    description: (
      <>
        Drop-in replacement for common PostgreSQL use cases. ACID transactions,
        MVCC, WAL, and crash recovery included.
      </>
    ),
  },
  {
    title: 'TypeScript First',
    emoji: '📘',
    description: (
      <>
        Full TypeScript support with type definitions. Modern async/await API,
        promise-based, and works with Node.js 16+.
      </>
    ),
  },
];

function Feature({title, emoji, description}: FeatureItem) {
  return (
    <div className={clsx('col col--4')}>
      <div className="text--center">
        <div className={styles.featureEmoji}>{emoji}</div>
      </div>
      <div className="text--center padding-horiz--md">
        <Heading as="h3">{title}</Heading>
        <p>{description}</p>
      </div>
    </div>
  );
}

export default function HomepageFeatures(): JSX.Element {
  return (
    <section className={styles.features}>
      <div className="container">
        <div className="row">
          {FeatureList.map((props, idx) => (
            <Feature key={idx} {...props} />
          ))}
        </div>
      </div>
    </section>
  );
}
