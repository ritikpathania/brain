# Feature Correlation & Redundancy Analysis

> [!IMPORTANT]
> Controlled benchmarks intentionally exaggerate feature influence to verify ranking behavior.
> Correlation indicates statistical association only. Highly correlated features may still encode distinct causal information.

Method: **Spearman** | Threshold: **0.70** | Total Candidates Checked: **72**

## Correlation Matrix

| Feature | access_frequency | freshness_decay | graph_degree | importance | lexical_similarity | provenance_confidence | recency | semantic_similarity |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| access_frequency | 1.0000 | 0.4264 | 0.3674 | 0.0000 | 0.0903 | 0.0000 | 0.0909 | 0.1343 |
| freshness_decay | 0.4264 | 1.0000 | 0.9847 | 0.0000 | 0.0759 | 0.0000 | 0.2132 | 0.3149 |
| graph_degree | 0.3674 | 0.9847 | 1.0000 | 0.0000 | 0.0636 | 0.0000 | 0.2099 | 0.3101 |
| importance | 0.0000 | 0.0000 | 0.0000 | 1.0000 | 0.0000 | 0.0000 | -0.7385 | 0.0000 |
| lexical_similarity | 0.0903 | 0.0759 | 0.0636 | 0.0000 | 1.0000 | 0.0000 | 0.0256 | -0.0472 |
| provenance_confidence | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 1.0000 | 0.0000 | 0.0000 |
| recency | 0.0909 | 0.2132 | 0.2099 | -0.7385 | 0.0256 | 0.0000 | 1.0000 | -0.1343 |
| semantic_similarity | 0.1343 | 0.3149 | 0.3101 | 0.0000 | -0.0472 | 0.0000 | -0.1343 | 1.0000 |

## Redundancy Alerts

| Feature A | Feature B | Correlation | Alert Level |
| :--- | :--- | ---: | :---: |
| freshness_decay | graph_degree | 0.9847 | ⚠️ Highly Correlated |
| importance | recency | -0.7385 | ⚠️ Highly Correlated |
