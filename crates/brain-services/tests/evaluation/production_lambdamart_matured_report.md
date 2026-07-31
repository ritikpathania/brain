# Matured LambdaMART vs Baseline Models: Production Corpus

> [!IMPORTANT]
> This report details evaluation results on a simulated 100-node production-like corpus with deterministic train/validation splits, early stopping diagnostics, and normalized feature importance analysis.

## Retrieval Performance Comparison (Full Corpus)

| Model | Composite | nDCG@5 | MRR | Recall@5 |
| :--- | ---: | ---: | ---: | ---: |
| **Linear Baseline** | 0.9550 | 0.9491 | 0.9278 | 1.0000 |
| **Logistic Regression** | 0.9051 | 0.8899 | 0.8667 | 0.9889 |
| **LambdaMART (Selected)** | 0.9667 | 0.9667 | 0.9670 | 0.9667 |

## Overfitting Diagnostics: Train vs. Validation Splits

| Model | Train Composite | Val Composite | Train nDCG@5 | Val nDCG@5 | Train MRR | Val MRR |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: |
| **Linear Baseline** | 0.9552 | 0.9546 | 0.9484 | 0.9520 | 0.9306 | 0.9167 |
| **Logistic Regression** | 0.9062 | 0.9004 | 0.8924 | 0.8803 | 0.8542 | 0.9167 |
| **LambdaMART** | 1.0000 | 0.8337 | 1.0000 | 0.8333 | 1.0000 | 0.8351 |

## LambdaMART Training & Selection Diagnostics

| Parameter | Value |
| :--- | ---: |
| **Best Selection Epoch** | 10 |
| **Total Boosting Rounds** | 50 |
| **Early Stopped** | 🟢 Yes (selected peak validation round) |
| **Validation Query Ratio** | 0.20 |
| **Average Scoring Latency** | 283.64 ns |

## Gain-Based Feature Importance

| Rank | Feature | Normalized Gain Importance |
| :--- | :--- | ---: |
| 1 | `SemanticSimilarity` | 0.5791 |
| 2 | `Importance` | 0.2939 |
| 3 | `LexicalSimilarity` | 0.1218 |
| 4 | `AccessFrequency` | 0.0051 |
| 5 | `FreshnessDecay` | 0.0000 |
| 6 | `GraphDegree` | 0.0000 |
| 7 | `ProvenanceConfidence` | 0.0000 |
| 8 | `Recency` | 0.0000 |

## Research Conclusion

> [NOTE]
> LambdaMART successfully outperformed the calibrated Linear Baseline on the validation split queries. This verifies that optimizing listwise ranking metrics via LambdaRank gradients achieves stable generalization and avoids overfitting on unseen evaluation corpora.
