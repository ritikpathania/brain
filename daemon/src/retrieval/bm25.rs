use std::collections::HashMap;

use crate::plugins::RetrievalAlgorithm;
use crate::retrieval::fuzzy::tokenize;
use crate::stm::TempNode;

pub struct Bm25Retrieval {
    pub k1: f32,
    pub b: f32,
}

impl Default for Bm25Retrieval {
    fn default() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }
}

impl Bm25Retrieval {
    pub fn new(k1: f32, b: f32) -> Self {
        Self { k1, b }
    }

    /// Score a corpus of nodes against a query using BM25
    pub fn score_corpus(&self, query: &str, corpus: &[TempNode]) -> Vec<(TempNode, i64)> {
        if corpus.is_empty() {
            return Vec::new();
        }

        let query_tokens: Vec<String> = tokenize(query).into_iter().collect();
        if query_tokens.is_empty() {
            return Vec::new();
        }

        let n = corpus.len() as f32;

        // 1. Tokenize each document and compute document lengths
        let mut doc_tokens = Vec::with_capacity(corpus.len());
        let mut doc_lengths = Vec::with_capacity(corpus.len());
        let mut total_length = 0.0;

        for node in corpus {
            let tokens = tokenize(&node.content);
            let len = tokens.len() as f32;
            total_length += len;

            // Compute term frequency for this document
            let mut tf = HashMap::new();
            for token in tokens {
                *tf.entry(token).or_insert(0.0) += 1.0;
            }
            doc_tokens.push(tf);
            doc_lengths.push(len);
        }

        let avgdl = total_length / n;

        // 2. Compute document frequency for each query term
        let mut df = HashMap::new();
        for token in &query_tokens {
            let mut count = 0.0;
            for tf_map in &doc_tokens {
                if tf_map.contains_key(token) {
                    count += 1.0;
                }
            }
            df.insert(token.clone(), count);
        }

        // 3. Score each document
        let mut scored = Vec::new();
        for (idx, node) in corpus.iter().enumerate() {
            let mut score = 0.0;
            let doc_len = doc_lengths[idx];
            let tf_map = &doc_tokens[idx];

            for token in &query_tokens {
                let df_val = *df.get(token).unwrap_or(&0.0);

                // Inverse Document Frequency (IDF) with BM25 smoothing
                let idf = ((n - df_val + 0.5) / (df_val + 0.5) + 1.0).ln();

                let tf_val = *tf_map.get(token).unwrap_or(&0.0);

                // BM25 term frequency scaling
                let numerator = tf_val * (self.k1 + 1.0);
                let denominator = tf_val
                    + self.k1
                        * (1.0 - self.b
                            + self.b * (doc_len / if avgdl > 0.0 { avgdl } else { 1.0 }));

                score += idf * (numerator / denominator);
            }

            if score > 0.0 {
                // Scale score to i64 (e.g. multiply by 100)
                let scaled_score = (score * 100.0) as i64;
                scored.push((node.clone(), scaled_score));
            }
        }

        scored
    }
}

impl RetrievalAlgorithm for Bm25Retrieval {
    fn name(&self) -> &str {
        "bm25"
    }

    fn retrieve(
        &self,
        query: &str,
        _index: &crate::stm::STMIndex,
        window: &[TempNode],
    ) -> Result<Vec<(TempNode, i64)>, String> {
        Ok(self.score_corpus(query, window))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bm25_scoring_relevance() {
        let retrieval = Bm25Retrieval::default();

        let corpus = vec![
            TempNode {
                id: "node1".to_string(),
                epoch: 0,
                content: "database setup and sqlite configuration".to_string(),
                timestamp: 100,
            },
            TempNode {
                id: "node2".to_string(),
                epoch: 0,
                content: "writing simple rust code without databases".to_string(),
                timestamp: 101,
            },
            TempNode {
                id: "node3".to_string(),
                epoch: 0,
                content: "sqlite is a file based lightweight relational database".to_string(),
                timestamp: 102,
            },
        ];

        // Querying for "sqlite database" should rank node1 and node3 higher than node2
        let results = retrieval.score_corpus("sqlite database", &corpus);
        assert!(!results.is_empty());
        assert_eq!(results.len(), 2);

        // Node 1 and Node 3 contain both keywords, Node 2 contains neither (or just "databases" which normalizes to "database")
        let first_id = &results[0].0.id;
        assert!(first_id == "node1" || first_id == "node3");
    }
}
