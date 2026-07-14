# Feature Impact & Sensitivity Report

Evaluated **24** queries containing a total of **264** candidates.

## Feature Invariants & Ablation Metrics

| Feature | Mean | Std Dev | Min | Max | Contribution | Avg Rank Shift | Queries Changed | Candidates Shifted % | Status |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | :---: | ---: | :---: |
| access_frequency | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.00% | 0.0000 | 0/24 | 0.0% | ⚠️ Zero Variance |
| freshness_decay | 0.1220 | 0.2581 | 0.0003 | 0.6696 | 0.00% | 0.0000 | 0/24 | 0.0% | ✓ Active |
| graph_degree | 0.1260 | 0.2673 | 0.0000 | 0.6931 | 0.00% | 0.0000 | 0/24 | 0.0% | ✓ Active |
| importance | 0.5000 | 0.3162 | 0.0000 | 1.0000 | 12.76% | 3.9091 | 24/24 | 82.6% | ✓ Active |
| lexical_similarity | 0.4715 | 1.3410 | 0.0000 | 7.2134 | 9.20% | 1.6212 | 23/24 | 66.3% | ✓ Active |
| provenance_confidence | 1.0000 | 0.0000 | 1.0000 | 1.0000 | 0.00% | 0.0000 | 0/24 | 0.0% | ⚠️ Zero Variance |
| recency | 0.9449 | 0.0342 | 0.8917 | 1.0000 | 25.08% | 0.0000 | 0/24 | 0.0% | ✓ Active |
| semantic_similarity | 1.0000 | 0.0000 | 1.0000 | 1.0000 | 52.97% | 0.0000 | 0/24 | 0.0% | ⚠️ Zero Variance |

## Zero Variance Features Summary

The following features are dormant (zero variance) and candidates for corpus enrichment:
- `access_frequency`
- `provenance_confidence`
- `semantic_similarity`
