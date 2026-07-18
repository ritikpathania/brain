# Feature Pruning & Degradation Report

> [!IMPORTANT]
> Controlled benchmarks intentionally exaggerate feature influence to verify ranking behavior.
> Disabling a critical feature followed by optimal grid recalibration measures the maximum degradation risk.

## Baseline Calibrated Setup

- Objective Metric: **Composite**
- Baseline Composite Score: **0.9253** (nDCG@5: **0.8756**, MRR: **1.0000**, Recall@5: **1.0000**)
- Baseline Weights: `lexical=1.00, semantic=0.00, recency=0.00, importance=0.00, provenance=1.00, graph=0.00, access=1.00, freshness=0.00`

## Recalibrated Ablation Matrix

| Pruned Feature | Baseline Weight | Retrained Composite | Composite Δ | Retrained nDCG@5 | MRR | Recall@5 | Cost (Candidates) | Impact |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: |
| **lexical_similarity** | 1.00 | 0.4198 | -0.5055 | 0.3463 | 0.3935 | 0.6667 | 128 | 🔴 Critical |
| **access_frequency** | 1.00 | 0.9113 | -0.0140 | 0.8522 | 1.0000 | 1.0000 | 128 | 🟠 Moderate Impact |
| **freshness_decay** | 0.00 | 0.9253 | +0.0000 | 0.8756 | 1.0000 | 1.0000 | 128 | 🟢 Safe to Prune |
| **graph_degree** | 0.00 | 0.9253 | +0.0000 | 0.8756 | 1.0000 | 1.0000 | 128 | 🟢 Safe to Prune |
| **importance** | 0.00 | 0.9253 | +0.0000 | 0.8756 | 1.0000 | 1.0000 | 128 | 🟢 Safe to Prune |
| **provenance_confidence** | 1.00 | 0.9253 | +0.0000 | 0.8756 | 1.0000 | 1.0000 | 128 | 🟢 Safe to Prune |
| **recency** | 0.00 | 0.9253 | +0.0000 | 0.8756 | 1.0000 | 1.0000 | 128 | 🟢 Safe to Prune |
| **semantic_similarity** | 0.00 | 0.9253 | +0.0000 | 0.8756 | 1.0000 | 1.0000 | 128 | 🟢 Safe to Prune |

## Optimal Ablated Weight Profiles

- **lexical_similarity** disabled: `lexical=0.00, semantic=0.00, recency=0.00, importance=1.00, provenance=1.00, graph=0.00, access=1.00, freshness=0.00`
- **access_frequency** disabled: `lexical=1.00, semantic=0.00, recency=0.00, importance=0.00, provenance=1.00, graph=0.00, access=0.00, freshness=0.00`
- **freshness_decay** disabled: `lexical=1.00, semantic=0.00, recency=0.00, importance=0.00, provenance=1.00, graph=0.00, access=1.00, freshness=0.00`
- **graph_degree** disabled: `lexical=1.00, semantic=0.00, recency=0.00, importance=0.00, provenance=1.00, graph=0.00, access=1.00, freshness=0.00`
- **importance** disabled: `lexical=1.00, semantic=0.00, recency=0.00, importance=0.00, provenance=1.00, graph=0.00, access=1.00, freshness=0.00`
- **provenance_confidence** disabled: `lexical=1.00, semantic=0.00, recency=0.00, importance=1.00, provenance=0.00, graph=0.00, access=1.00, freshness=0.00`
- **recency** disabled: `lexical=1.00, semantic=0.00, recency=0.00, importance=0.00, provenance=1.00, graph=0.00, access=1.00, freshness=0.00`
- **semantic_similarity** disabled: `lexical=1.00, semantic=0.00, recency=0.00, importance=0.00, provenance=1.00, graph=0.00, access=1.00, freshness=0.00`
