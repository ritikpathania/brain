# Feature Correlation & Redundancy Analysis

> [!IMPORTANT]
> Controlled benchmarks intentionally exaggerate feature influence to verify ranking behavior.
> Correlation indicates statistical association only. Highly correlated features may still encode distinct causal information.

Method: **Pearson** | Threshold: **0.70** | Total Candidates Checked: **72**

## Correlation Matrix

| Feature | access_frequency | freshness_decay | graph_degree | importance | lexical_similarity | provenance_confidence | recency | semantic_similarity |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| access_frequency | 1.0000 | 0.4264 | 0.2748 | 0.0000 | 0.0407 | 0.0781 | 0.0909 | 0.1026 |
| freshness_decay | 0.4264 | 1.0000 | 0.9206 | 0.0000 | 0.0477 | 0.1831 | 0.2132 | 0.2407 |
| graph_degree | 0.2748 | 0.9206 | 1.0000 | 0.0000 | 0.0351 | 0.1685 | 0.1963 | 0.2216 |
| importance | 0.0000 | 0.0000 | 0.0000 | 1.0000 | 0.0000 | 0.0000 | -0.7385 | 0.0000 |
| lexical_similarity | 0.0407 | 0.0477 | 0.0351 | 0.0000 | 1.0000 | 0.0000 | 0.0000 | -0.0430 |
| provenance_confidence | 0.0781 | 0.1831 | 0.1685 | 0.0000 | 0.0000 | 1.0000 | -0.0781 | -0.0881 |
| recency | 0.0909 | 0.2132 | 0.1963 | -0.7385 | 0.0000 | -0.0781 | 1.0000 | -0.1026 |
| semantic_similarity | 0.1026 | 0.2407 | 0.2216 | 0.0000 | -0.0430 | -0.0881 | -0.1026 | 1.0000 |

## Redundancy Alerts

| Feature A | Feature B | Correlation | Alert Level |
| :--- | :--- | ---: | :---: |
| freshness_decay | graph_degree | 0.9206 | ⚠️ Highly Correlated |
| importance | recency | -0.7385 | ⚠️ Highly Correlated |
