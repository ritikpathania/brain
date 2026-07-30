use brain_domain::bkf::*;
use brain_services::reflection::pass_context::*;
use tokio_util::sync::CancellationToken;

struct MockSnapshot;

impl KnowledgeSnapshotView for MockSnapshot {
    fn entities(&self) -> &[KnowledgeEntity] { &[] }
    fn assertions(&self) -> &[SemanticAssertion] { &[] }
    fn predicates(&self) -> &[Predicate] { &[] }
    fn active_facts(&self) -> &[FactVersion] { &[] }
}

struct TestNoOpPass;

impl V2ReflectionPass for TestNoOpPass {
    fn id(&self) -> PassId {
        PassId::new("test_noop_pass")
    }

    fn dependencies(&self) -> &[PassId] {
        &[]
    }

    fn analyze(
        &self,
        _snapshot: &dyn KnowledgeSnapshotView,
        _context: &V2ReflectionContext,
    ) -> Result<Option<ReflectionOutcome>, String> {
        let plan = RewritePlan {
            pass_id: self.id(),
            reason: RewriteReason::Canonicalization,
            rationale: "No ops".to_string(),
            execution_cost: 0,
            operations: vec![],
        };
        Ok(Some(ReflectionOutcome {
            plan,
            diagnostics: vec![PassDiagnostic {
                severity: DiagnosticSeverity::Info,
                code: "NO_OP".to_string(),
                message: "No operations needed".to_string(),
            }],
        }))
    }
}

#[test]
fn test_v2_reflection_pass_interface() {
    let pass = TestNoOpPass;
    let snapshot = MockSnapshot;
    let context = V2ReflectionContext {
        now: Timestamp::now(),
        cancellation_token: CancellationToken::new(),
        max_operations_budget: 100,
    };

    let outcome = pass.analyze(&snapshot, &context).unwrap().unwrap();
    assert_eq!(outcome.plan.pass_id.as_str(), "test_noop_pass");
    assert_eq!(outcome.diagnostics.len(), 1);
    assert_eq!(outcome.diagnostics[0].code, "NO_OP");
}
