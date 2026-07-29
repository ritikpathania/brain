#![allow(missing_docs)]

use crate::runtime::aggregator::*;
use crate::runtime::events::*;
use crate::runtime::models::*;
use crate::runtime::repository::*;

pub struct RecoveryEngine<R: ExecutionRepository> {
    repo: R,
}

impl<R: ExecutionRepository> RecoveryEngine<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub fn recover_execution(
        &self,
        execution_id: ExecutionId,
    ) -> Result<Option<ExecutionProjection>, RepositoryError> {
        let header = match self.repo.get_execution_header(execution_id)? {
            Some(h) => h,
            None => return Ok(None),
        };

        let mut aggregator = ExecutionAggregator::new(header);
        let events = self.repo.get_journal_events(execution_id, SequenceNo(0))?;

        for event in &events {
            let _ = aggregator.apply(event);
        }

        Ok(Some(aggregator.projection().clone()))
    }
}
