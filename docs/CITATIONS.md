# NexusQL: Academic Citations & References

This document contains all academic papers, research citations, and technical references that inform NexusQL's neural and brain-inspired technologies.

---

## Hyperdimensional Computing (HDC)

### Foundational Papers

**[1] Kanerva, P. (2009)**
- **Title:** "Hyperdimensional Computing: An Introduction to Computing in Distributed Representation with High-Dimensional Random Vectors"
- **Published:** *Cognitive Computation*, Volume 1, Pages 139-159
- **DOI:** 10.1007/s12559-009-9009-8
- **Link:** https://link.springer.com/article/10.1007/s12559-009-9009-8
- **Key Contribution:** Introduced the foundational concepts of HDC, holographic representations, and operations (binding, bundling, permutation)
- **Used in NexusQL:** Core HDC engine design, hypervector operations

**[2] Yu, T., Zhang, Y., Zhang, Z., & De Sa, C. (2022)**
- **Title:** "Understanding Hyperdimensional Computing for Parallel Single-Pass Learning"
- **Published:** *arXiv:2202.04805* [cs.LG]
- **Date:** February 10, 2022 (revised January 4, 2023)
- **Link:** https://arxiv.org/abs/2202.04805
- **Key Contribution:** Theoretical analysis of HDC accuracy, introduced Random Fourier Features enhancement, finite group VSAs
- **Performance:** Up to 7.6% improvement over state-of-the-art HDC
- **Used in NexusQL:** Performance optimization strategies, understanding of HDC limitations

### Recent Advances (2024)

**[3] "Hyperdimensional computing with holographic and adaptive encoder" (2024)**
- **Published:** *Frontiers in Artificial Intelligence*
- **Link:** https://www.frontiersin.org/journals/artificial-intelligence/articles/10.3389/frai.2024.1371988/full
- **Key Contribution:** FLASH (Fast, Learnable, Adaptive, Stays Holographic) encoder using Random Fourier Features and Bochner's theorem
- **Used in NexusQL:** Planned FLASH encoder implementation for adaptive learning

**[4] "Editorial: Brain-inspired Hyperdimensional Computing: Algorithms, models, and architectures" (2022)**
- **Published:** *Frontiers in Neuroscience*
- **Link:** https://www.frontiersin.org/journals/neuroscience/articles/10.3389/fnins.2022.1102568/full
- **Used in NexusQL:** Understanding brain-inspired computing principles

### Survey Papers

**[5] "A Survey on Hyperdimensional Computing aka Vector Symbolic Architectures" (2022)**
- **Published:** *ACM Computing Surveys*
- **Link:** https://dl.acm.org/doi/10.1145/3558000
- **Key Contribution:** Comprehensive survey of HDC applications, cognitive models, and challenges

**[6] "Hyperdimensional computing in biomedical sciences: a brief review" (2025)**
- **Published:** *PeerJ Computer Science*
- **Link:** https://peerj.com/articles/cs-2885/
- **Used in NexusQL:** Understanding real-world HDC applications

---

## ColBERT (Contextualized Late Interaction)

### Original ColBERT

**[7] Khattab, O., & Zaharia, M. (2020)**
- **Title:** "ColBERT: Efficient and Effective Passage Search via Contextualized Late Interaction over BERT"
- **Published:** *SIGIR '20: Proceedings of the 43rd International ACM SIGIR Conference on Research and Development in Information Retrieval*, Pages 39-48
- **Date:** July 25-30, 2020, Virtual Event, China
- **DOI:** 10.1145/3397271.3401075
- **arXiv:** https://arxiv.org/abs/2004.12832
- **PDF:** https://people.eecs.berkeley.edu/~matei/papers/2020/sigir_colbert.pdf
- **Key Contribution:** Late interaction architecture, MaxSim operator, token-level embeddings
- **Performance:** 2 orders of magnitude faster than BERT, 4 orders of magnitude fewer FLOPs per query
- **Used in NexusQL:** Core ColBERT engine, MaxSim operator implementation

### ColBERTv2

**[8] Santhanam, K., Khattab, O., Saad-Falcon, J., Potts, C., & Zaharia, M. (2022)**
- **Title:** "ColBERTv2: Effective and Efficient Retrieval via Lightweight Late Interaction"
- **Published:** *NAACL 2022*
- **arXiv:** https://arxiv.org/abs/2112.01488
- **PDF:** https://arxiv.org/pdf/2112.01488
- **Key Contribution:** Residual compression (6-10x reduction), denoised supervision
- **Used in NexusQL:** Compression techniques for production deployment (planned)

### Related ColBERT Research

**[9] "ColBERT: A complete guide" (Medium)**
- **Link:** https://medium.com/@varun030403/colbert-a-complete-guide-1552468335ae
- **Used in NexusQL:** Implementation guidance

**[10] "An Overview of Late Interaction Retrieval Models: ColBERT, ColPali, and ColQwen" (Weaviate)**
- **Link:** https://weaviate.io/blog/late-interaction-overview
- **Used in NexusQL:** Understanding late interaction architectures

---

## HNSW (Hierarchical Navigable Small World)

### Original Paper

**[11] Malkov, Y.A., & Yashunin, D.A. (2016)**
- **Title:** "Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small World graphs"
- **Published:** *IEEE Transactions on Pattern Analysis and Machine Intelligence*
- **arXiv:** https://arxiv.org/abs/1603.09320
- **PDF:** https://arxiv.org/pdf/1603.09320
- **Key Contribution:** Hierarchical graph construction with exponentially decaying layer probability, M parameter optimization
- **Formula:** l = ⌊-ln(unif(0,1)) × mL⌋, where mL = 1/ln(M)
- **Complexity:** O(log N) search time
- **Used in NexusQL:** Core HNSW index implementation in src/index/hnsw.rs

### HNSW Technical Resources

**[12] "Understanding Hierarchical Navigable Small Worlds (HNSW)" (Zilliz)**
- **Link:** https://zilliz.com/learn/hierarchical-navigable-small-worlds-HNSW
- **Used in NexusQL:** Implementation best practices

**[13] "A Visual Guide to the Hierarchical Navigable Small World Algorithm" (2024)**
- **Link:** https://cfu288.com/blog/2024-05_visual-guide-to-hnsw/
- **Used in NexusQL:** Understanding multi-layer graph structure

**[14] "Hierarchical Navigable Small Worlds (HNSW)" (Pinecone)**
- **Link:** https://www.pinecone.io/learn/series/faiss/hnsw/
- **Used in NexusQL:** Performance characteristics and parameter tuning

---

## BERT (Bidirectional Encoder Representations from Transformers)

### Original Paper

**[15] Devlin, J., Chang, M.-W., Lee, K., & Toutanova, K. (2018)**
- **Title:** "BERT: Pre-training of Deep Bidirectional Transformers for Language Understanding"
- **Published:** *arXiv:1810.04805* [cs.CL]
- **Date:** October 11, 2018
- **Link:** https://arxiv.org/abs/1810.04805
- **Google Research:** https://research.google/pubs/pub47751/
- **Key Contribution:** Bidirectional pre-training, masked language modeling, next sentence prediction
- **Architecture:** BERT-Base: 12 layers, 768 hidden units, 12 attention heads (~110M parameters)
- **Used in NexusQL:** Foundation for ColBERT token embeddings

### BERT Resources

**[16] "The Illustrated BERT" (Jay Alammar)**
- **Link:** https://jalammar.github.io/illustrated-bert/
- **Used in NexusQL:** Understanding transformer architecture and contextualized representations

---

## Neuro-Symbolic AI

### Recent Surveys (2024-2025)

**[17] "Neuro-Symbolic AI in 2024: A Systematic Review" (2025)**
- **Published:** *arXiv:2501.05435*
- **Link:** https://arxiv.org/abs/2501.05435
- **Key Contribution:** Comprehensive review of 2024 neuro-symbolic research, integration approaches
- **Used in NexusQL:** Future roadmap for neuro-symbolic module (v0.6.0)

**[18] "A review of neuro-symbolic AI integrating reasoning and learning for advanced cognitive systems" (2025)**
- **Published:** *Data Science and Management*
- **Link:** https://www.sciencedirect.com/science/article/pii/S2667305325000675
- **Key Contribution:** Integration of neural learning with symbolic reasoning
- **Used in NexusQL:** Architectural planning for logic tensor networks

### Logic Tensor Networks

**[19] Serafini, L., & d'Avila Garcez, A. (2016)**
- **Title:** "Logic Tensor Networks: Deep Learning and Logical Reasoning from Data and Knowledge"
- **Link:** https://www.researchgate.net/publication/309708192_Learning_and_Reasoning_with_Logic_Tensor_Networks
- **Key Contribution:** Encoding logical formulas as neural networks
- **Used in NexusQL:** Planned LTN implementation for explainable reasoning

**[20] "Neuro-Symbolic AI: Explainability, Challenges, and Future Trends" (2024)**
- **Published:** *arXiv:2411.04383*
- **Link:** https://arxiv.org/abs/2411.04383
- **Used in NexusQL:** Understanding explainability benefits

---

## SPLADE (Sparse Lexical and Expansion)

### Original Papers

**[21] "SPLADE: Sparse Lexical and Expansion Model for First Stage Ranking" (2021)**
- **Published:** *SIGIR 2021*
- **Organization:** Naver Labs Europe
- **Link:** https://europe.naverlabs.com/blog/splade-a-sparse-bi-encoder-bert-based-model-achieves-effective-and-efficient-first-stage-ranking/
- **Key Contribution:** Learned sparse embeddings combining keyword search with neural understanding
- **Formula:** wⱼ = Σᵢ∈t log(1 + ReLU(wᵢⱼ))
- **Used in NexusQL:** Planned hybrid search implementation (v0.2.0)

**[22] "SPLADE v2: Sparse Lexical and Expansion Model for Information Retrieval" (2021)**
- **Published:** *arXiv:2109.10086*
- **Link:** https://arxiv.org/abs/2109.10086
- **Key Contribution:** FLOPS regularization for efficiency
- **Formula:** FLOPS(q, d) = ||q||₀ × ||d||₀
- **Used in NexusQL:** Efficiency optimizations

### SPLADE Resources

**[23] "SPLADE for Sparse Vector Search Explained" (Pinecone)**
- **Link:** https://www.pinecone.io/learn/splade/
- **Used in NexusQL:** Implementation guidance

**[24] "Elastic Learned Sparse Encoder" (Elastic, 2023)**
- **Link:** https://www.elastic.co/search-labs/blog/elastic-learned-sparse-encoder-elser-retrieval-performance
- **Used in NexusQL:** Production deployment insights

---

## Graph Neural Networks (GNNs)

### Knowledge Graph Embeddings

**[25] Bordes, A., Usunier, N., Garcia-Duran, A., Weston, J., & Yakhnenko, O. (2013)**
- **Title:** "Translating Embeddings for Modeling Multi-relational Data"
- **Published:** *NeurIPS 2013*
- **Key Contribution:** TransE algorithm (h + r ≈ t)
- **Used in NexusQL:** Planned knowledge graph support (v0.6.0)

**[26] Kipf, T.N., & Welling, M. (2016)**
- **Title:** "Semi-Supervised Classification with Graph Convolutional Networks"
- **Published:** *ICLR 2017*
- **Key Contribution:** Graph convolutional networks (GCN), message passing
- **Used in NexusQL:** Future GNN engine architecture

### GNN Resources

**[27] "Creating Embeddings from Knowledge Graphs Using Graph Neural Networks"**
- **Link:** https://medium.com/@busra.oguzoglu/creating-embeddings-from-knowledge-graphs-using-graph-neural-networks-ffc6cc62275c
- **Used in NexusQL:** Understanding GNN-based embeddings

---

## Quantum Machine Learning

### Quantum Embeddings

**[28] Tang, Y., et al. (2022)**
- **Title:** "Quantum Embedding Search for Quantum Machine Learning"
- **Published:** *IEEE Access*
- **arXiv:** https://arxiv.org/abs/2105.11853
- **Key Contribution:** Search for optimal quantum embedding design
- **Used in NexusQL:** Future quantum module exploration (v2.0.0+)

**[29] Lloyd, S., Schuld, M., et al. (2020)**
- **Title:** "Quantum embeddings for machine learning"
- **Published:** *arXiv:2001.03622*
- **Link:** https://arxiv.org/abs/2001.03622
- **Key Contribution:** Quantum metric learning, exponential Hilbert space
- **Used in NexusQL:** Understanding quantum advantage potential

### Quantum Resources

**[30] "Experimental Quantum Embedding for Machine Learning" (2022)**
- **Published:** *Advanced Quantum Technologies*
- **Link:** https://advanced.onlinelibrary.wiley.com/doi/full/10.1002/qute.202100140
- **Used in NexusQL:** Experimental validation insights

---

## Database Systems & Storage

### LSM Trees & RocksDB

**[31] "RocksDB: Evolution of Development Priorities in a Key-value Store" (2021)**
- **Published:** *ACM TODS*
- **Link:** https://dl.acm.org/doi/fullHtml/10.1145/3483840
- **Used in NexusQL:** LSM tree storage layer design

**[32] "Compaction" (RocksDB Wiki)**
- **Link:** https://github.com/facebook/rocksdb/wiki/Compaction
- **Used in NexusQL:** Compaction strategies (leveled, tiered)

**[33] "Write Ahead Log" (RocksDB Wiki)**
- **Link:** https://github.com/facebook/rocksdb/wiki/Write-Ahead-Log
- **Used in NexusQL:** WAL implementation for durability

### Vectorized Execution

**[34] "DuckDB: Vector Format" (Documentation)**
- **Link:** https://duckdb.org/docs/internals/vector
- **Used in NexusQL:** Vectorized query execution (planned)

**[35] "ClickHouse Architecture"**
- **Link:** https://clickhouse.com/docs/en/development/architecture
- **Used in NexusQL:** Columnar storage and OLAP optimizations (planned)

### PostgreSQL Internals

**[36] PostgreSQL Documentation (v16)**
- **Link:** https://www.postgresql.org/docs/16/
- **Used in NexusQL:** MVCC, transaction isolation, query planning

---

## Performance & Benchmarking

**[37] "TPC-H Benchmark"**
- **Link:** https://www.tpc.org/tpch/
- **Used in NexusQL:** Performance validation target

**[38] "Understanding MVCC in PostgreSQL"**
- **Link:** https://www.postgresql.org/docs/current/mvcc.html
- **Used in NexusQL:** MVCC implementation design

---

## Additional Resources

### Academic Databases

- **arXiv.org** - https://arxiv.org/ (Pre-print papers)
- **ACM Digital Library** - https://dl.acm.org/
- **IEEE Xplore** - https://ieeexplore.ieee.org/
- **Google Scholar** - https://scholar.google.com/
- **Semantic Scholar** - https://www.semanticscholar.org/

### Technical Blogs & Tutorials

- **Pinecone Learning Center** - https://www.pinecone.io/learn/
- **Weaviate Blog** - https://weaviate.io/blog/
- **Zilliz Learn** - https://zilliz.com/learn/
- **Elastic Search Labs** - https://www.elastic.co/search-labs/

---

## Citation Count by Technology

| Technology | Citations | Status in NexusQL |
|------------|-----------|-------------------|
| **HDC** | 6 papers | ✅ Implemented |
| **ColBERT** | 4 papers | ✅ Implemented |
| **HNSW** | 4 papers | ✅ Implemented |
| **BERT** | 2 papers | ✅ Foundation |
| **Neuro-Symbolic** | 4 papers | 🔄 Planned (v0.6.0) |
| **SPLADE** | 4 papers | 🔄 Planned (v0.2.0) |
| **GNN** | 3 papers | 🔄 Planned (v0.6.0) |
| **Quantum** | 3 papers | 🔬 Research (v2.0.0+) |
| **Database Systems** | 5 papers | ✅ Foundation |

**Total Citations:** 38+ academic papers and technical resources

---

## How to Cite NexusQL

If you use NexusQL in your research, please cite:

```bibtex
@software{nexusql2025,
  author = {Nguyen, John},
  title = {NexusQL: Next-Generation Database with Brain-Inspired AI},
  year = {2025},
  url = {https://github.com/johnhnguyen97/NexusQL},
  note = {PostgreSQL-compatible database with Hyperdimensional Computing,
          ColBERT late interaction, and HNSW vector search}
}
```

---

## License Notes

All cited papers are used for research and educational purposes in accordance with fair use principles. NexusQL implements novel combinations of these technologies but does not directly copy code from closed-source implementations.

---

**Last Updated:** November 2, 2025
**Document Maintainer:** John Nguyen
**Total References:** 38+
**Version:** 1.0
