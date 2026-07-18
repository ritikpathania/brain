# Feature Impact & Sensitivity Report

Evaluated **6** queries containing a total of **72** candidates.

## Feature Invariants & Ablation Metrics

| Feature | Mean | Std Dev | Min | Max | Contribution | Avg Rank Shift | Queries Changed | Candidates Shifted % | Status |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | :---: | ---: | :---: |
| access_frequency | 0.1998 | 0.6627 | 0.0000 | 2.3979 | 1.39% | 0.5278 | 6/6 | 34.7% | ✓ Active |
| freshness_decay | 0.3336 | 0.4712 | 0.0003 | 1.0000 | 2.63% | 0.2222 | 3/6 | 15.3% | ✓ Active |
| graph_degree | 0.2888 | 0.4437 | 0.0000 | 1.3863 | 2.28% | 0.4167 | 6/6 | 27.8% | ✓ Active |
| importance | 0.5000 | 0.2041 | 0.0000 | 1.0000 | 5.22% | 0.4167 | 6/6 | 16.7% | ✓ Active |
| lexical_similarity | 0.7411 | 1.6197 | 0.0000 | 5.2371 | 3.27% | 0.7778 | 6/6 | 43.1% | ✓ Active |
| provenance_confidence | 0.8417 | 0.2253 | 0.1000 | 1.0000 | 8.81% | 0.8056 | 6/6 | 48.6% | ✓ Active |
| recency | 0.9637 | 0.1206 | 0.5638 | 1.0000 | 10.13% | 0.1389 | 5/6 | 13.9% | ✓ Active |
| semantic_similarity | 0.9250 | 0.2203 | 0.2000 | 1.0000 | 66.26% | 1.1944 | 6/6 | 56.9% | ✓ Active |

## Zero Variance Features Summary

None. All features show non-zero variance across the candidate set.
