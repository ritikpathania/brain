# Supervised Logistic Regression Ranker Report

> [!IMPORTANT]
> Controlled benchmarks intentionally exaggerate feature influence to verify ranking behavior.
> Model calibration is calculated on a deterministic training dataset extracted directly from EvaluationSession.

## Optimizer Convergence & Diagnostics

| Metric | Value |
| :--- | ---: |
| Initial BCE Loss | 0.693147 |
| Final BCE Loss | 0.077766 |
| Epochs Executed | 1000 |
| Converged | 🔴 No (reached epoch limit; loss was still decreasing) |
| L2 Regularization (λ) | 0.0010 |
| Model Intercept (b) | -1.8572 |

## Feature Parameter Comparison

| Feature Name | Linear Calibrated Weight | Logistic Trained Weight |
| :--- | ---: | ---: |
| access_frequency | 1.0000 | -0.3474 |
| freshness_decay | 0.0000 | -0.7213 |
| graph_degree | 0.0000 | -0.5340 |
| importance | 0.0000 | -0.5962 |
| lexical_similarity | 1.0000 | 2.0792 |
| provenance_confidence | 1.0000 | -0.5277 |
| recency | 0.0000 | -1.0261 |
| semantic_similarity | 0.0000 | -0.3763 |

## Retrieval Performance Baseline comparison

| Model Type | Composite Score | nDCG@5 | MRR | Recall@5 |
| :--- | ---: | ---: | ---: | ---: |
| **Linear Baseline** | 0.9253 | 0.8756 | 1.0000 | 1.0000 |
| **Logistic Regression** | 0.8345 | 0.7798 | 1.0000 | 0.8333 |

## Research Conclusion

> [!NOTE]
> On the current controlled corpus, the Logistic Regression model trained with pointwise BCE did not outperform the Linear Baseline that was directly calibrated for the Composite objective. This is consistent with the broader observation that optimizing a pointwise objective does not necessarily maximize listwise ranking metrics.
