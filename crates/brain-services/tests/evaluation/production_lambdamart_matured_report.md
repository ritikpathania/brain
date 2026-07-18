# Matured LambdaMART vs Baseline Models: Production Corpus

> [!IMPORTANT]
> This report details evaluation results on a simulated 100-node production-like corpus with deterministic train/validation splits, early stopping diagnostics, and normalized feature importance analysis.

## Retrieval Performance Comparison (Full Corpus)

| Model | Composite | nDCG@5 | MRR | Recall@5 |
| :--- | ---: | ---: | ---: | ---: |
| **Linear Baseline** | 0.9550 | 0.9491 | 0.9278 | 1.0000 |
| **Logistic Regression** | 0.9051 | 0.8899 | 0.8667 | 0.9889 |
| **LambdaMART (Selected)** | 0.9546 | 0.9493 | 0.9361 | 0.9889 |

## Overfitting Diagnostics: Train vs. Validation Splits

| Model | Train Composite | Val Composite | Train nDCG@5 | Val nDCG@5 | Train MRR | Val MRR |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: |
| **Linear Baseline** | 0.9552 | 0.9546 | 0.9484 | 0.9520 | 0.9306 | 0.9167 |
| **Logistic Regression** | 0.9062 | 0.9004 | 0.8924 | 0.8803 | 0.8542 | 0.9167 |
| **LambdaMART** | 0.9795 | 0.8547 | 0.9763 | 0.8412 | 0.9688 | 0.8056 |

## LambdaMART Training & Selection Diagnostics

| Parameter | Value |
| :--- | ---: |
| **Best Selection Epoch** | 1 |
| **Total Boosting Rounds** | 50 |
| **Early Stopped** | 🟢 Yes (selected peak validation round) |
| **Validation Query Ratio** | 0.20 |
| **Average Scoring Latency** | 57.90 ns |

## Gain-Based Feature Importance

| Rank | Feature | Normalized Gain Importance |
| :--- | :--- | ---: |
| 1 | `SemanticSimilarity` | 0.5993 |
| 2 | `Importance` | 0.3793 |
| 3 | `LexicalSimilarity` | 0.0214 |
| 4 | `AccessFrequency` | 0.0000 |
| 5 | `FreshnessDecay` | 0.0000 |
| 6 | `GraphDegree` | 0.0000 |
| 7 | `ProvenanceConfidence` | 0.0000 |
| 8 | `Recency` | 0.0000 |

## Research Conclusion

> [NOTE]
> LambdaMART did not outperform the calibrated Linear Baseline on validation queries. This suggests that the current calibration baseline represents a highly robust model for this controlled vocabulary scope.
