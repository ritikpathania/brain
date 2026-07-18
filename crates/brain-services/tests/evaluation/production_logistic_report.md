# Supervised Logistic Regression vs Linear Baseline: Production Corpus

> [!IMPORTANT]
> This report details evaluation results on a simulated 100-node production-like corpus with realistic relation edges and temporal access frequency metrics.

## Retrieval Performance Comparison

| Metric | Linear Baseline | Logistic Regression | Delta (\(\Delta\)) |
| :--- | ---: | ---: | ---: |
| **Composite** | 0.9550 | 0.9051 | -0.0500 |
| **nDCG@5** | 0.9491 | 0.8899 | -0.0592 |
| **MRR** | 0.9278 | 0.8667 | -0.0611 |
| **Recall@5** | 1.0000 | 0.9889 | -0.0111 |

## Query-Level Delta Significance

| Outcome | Count |
| :--- | ---: |
| **Queries Improved** | 2 |
| **Queries Unchanged** | 20 |
| **Queries Degraded** | 8 |

## Optimizer Convergence & Diagnostics

| Parameter | Value |
| :--- | ---: |
| Initial BCE Loss | 0.693147 |
| Final BCE Loss | 0.025548 |
| Epochs Executed | 1000 |
| Converged | 🔴 No (reached epoch limit; loss was still decreasing) |
| L2 Regularization (λ) | 0.0010 |
| Model Intercept (b) | -2.0229 |

## Learned Parameters Comparison

| Feature Name | Linear Calibrated Weight | Logistic Trained Weight |
| :--- | ---: | ---: |
| access_frequency | 0.0000 | 0.5048 |
| freshness_decay | 0.0000 | -1.3755 |
| graph_degree | 0.0000 | -0.9534 |
| importance | 0.0000 | 0.0566 |
| lexical_similarity | 0.0000 | 0.2982 |
| provenance_confidence | 1.0000 | -0.7066 |
| recency | 0.0000 | -1.3755 |
| semantic_similarity | 1.0000 | 0.1315 |

## Research Conclusion

> [NOTE]
> On the current 100-node production-like corpus, the Logistic Regression model trained with pointwise BCE did not outperform the Linear Baseline that was directly calibrated for the Composite objective. This is consistent with the broader observation that optimizing a pointwise objective does not necessarily maximize listwise ranking metrics.
