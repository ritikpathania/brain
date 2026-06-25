use crate::plugins::RankingStrategy;
use crate::stm::TempNode;

#[derive(Clone)]
pub struct DefaultRanking;

impl RankingStrategy for DefaultRanking {
    fn name(&self) -> &str {
        "default"
    }

    fn rank(&self, _query: &str, candidates: &mut Vec<(TempNode, i64)>) -> Result<(), String> {
        // Sort by score descending (highest score first), then by timestamp descending
        candidates.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| b.0.timestamp.cmp(&a.0.timestamp))
        });
        Ok(())
    }
}
