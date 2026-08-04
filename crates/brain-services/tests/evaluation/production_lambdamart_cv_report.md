# 5-Fold Cross-Validation Robustness Report

> [!IMPORTANT]
> This report details evaluation results on a simulated 100-node production-like corpus under 5-Fold Cross-Validation, establishing split-independent performance spreads (Mean, Std Dev, Min, Max).

## Fold Balance Composition

| Fold | Train Queries Count | Validation Queries Count |
| :--- | ------------------: | -----------------------: |
| 0 | 24 | 6 |
| 1 | 24 | 6 |
| 2 | 24 | 6 |
| 3 | 24 | 6 |
| 4 | 24 | 6 |

## Validation Query Partitions

### Fold 0 Validation Queries
- q_p001, q_p006, q_p011, q_p016, q_p021, q_p026

### Fold 1 Validation Queries
- q_p002, q_p007, q_p012, q_p017, q_p022, q_p027

### Fold 2 Validation Queries
- q_p003, q_p008, q_p013, q_p018, q_p023, q_p028

### Fold 3 Validation Queries
- q_p004, q_p009, q_p014, q_p019, q_p024, q_p029

### Fold 4 Validation Queries
- q_p005, q_p010, q_p015, q_p020, q_p025, q_p030

## Cross-Validation Metric Distributions Comparison

| Metric | Model | Mean | Std Dev | Min | Max |
| :--- | :--- | ---: | ---: | ---: | ---: |
| **Composite** | Linear Baseline | 0.9372 | 0.0591 | 0.8393 | 1.0000 |
| | Logistic Regression | 0.9051 | 0.0233 | 0.8929 | 0.9464 |
| | LambdaMART | 0.9319 | 0.0933 | 0.8254 | 1.0000 |
| **nDCG@5** | Linear Baseline | 0.9268 | 0.0676 | 0.8155 | 1.0000 |
| | Logistic Regression | 0.8899 | 0.0272 | 0.8770 | 0.9385 |
| | LambdaMART | 0.9289 | 0.0977 | 0.8109 | 1.0000 |
| **MRR** | Linear Baseline | 0.9167 | 0.1021 | 0.7500 | 1.0000 |
| | Logistic Regression | 0.8667 | 0.0456 | 0.8333 | 0.9167 |
| | LambdaMART | 0.9283 | 0.0987 | 0.8056 | 1.0000 |
| **Recall@5** | Linear Baseline | 0.9889 | 0.0248 | 0.9444 | 1.0000 |
| | Logistic Regression | 0.9889 | 0.0248 | 0.9444 | 1.0000 |
| | LambdaMART | 0.9444 | 0.0786 | 0.8333 | 1.0000 |

## Research Conclusion

> [NOTE]
> Under 5-Fold Cross-Validation, LambdaMART achieved a mean Composite score of **0.9319**, while the Linear Baseline maintained **0.9372** (Delta: **-0.0054**). This demonstrates that while LambdaMART learns complex local non-linear interactions, the linear model calibrated over multiple folds remains a very strong competitor for this corpus scale.
