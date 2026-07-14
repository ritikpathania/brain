/// Recall@K computation logic.
pub mod recall;
/// Precision@K computation logic.
pub mod precision;
/// Mean Reciprocal Rank (MRR) computation logic.
pub mod mrr;
/// Normalized Discounted Cumulative Gain (nDCG@K) computation logic.
pub mod ndcg;

pub use recall::compute_recall_at_k;
pub use precision::compute_precision_at_k;
pub use mrr::compute_mrr;
pub use ndcg::compute_ndcg_at_k;
