//! Data Transfer Object (DTO) mapper.

use brain_core::errors::BrainError;
use brain_domain::{Edge, EdgeDTO, MemoryDTO, Node, NodeDTO};

/// Maps a domain Node and its connections to a MemoryDTO.
pub fn to_memory_dto(node: &Node, connections: &[Edge]) -> Result<MemoryDTO, BrainError> {
    let mut incoming_edges = Vec::new();
    let mut outgoing_edges = Vec::new();

    for edge in connections {
        let edge_dto = EdgeDTO::new(
            edge.source.to_string(),
            edge.target.to_string(),
            edge.relation.to_string(),
            edge.weight,
        );
        if edge.target == node.id {
            incoming_edges.push(edge_dto);
        } else {
            outgoing_edges.push(edge_dto);
        }
    }

    let node_type_str = node.node_type.to_string();

    let node_dto = NodeDTO::new(
        node.id.to_string(),
        node.label.clone(),
        node_type_str,
        serde_json::to_value(&node.properties).unwrap_or_default(),
    );

    Ok(MemoryDTO::new(node_dto, incoming_edges, outgoing_edges))
}

use crate::reconciliation::{
    ContradictionRecord, DiagnosticSeverity, MergeProposal, PassDiagnostic, PassReport,
};
use brain_domain::{ContradictionRecordDTO, MergeProposalDTO, PassDiagnosticDTO, PassReportDTO};

/// Maps a PassDiagnostic domain item to a PassDiagnosticDTO.
pub fn to_pass_diagnostic_dto(diag: &PassDiagnostic) -> PassDiagnosticDTO {
    let severity = match diag.severity {
        DiagnosticSeverity::Info => "info",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Error => "error",
    };
    PassDiagnosticDTO {
        severity: severity.to_string(),
        code: diag.code.clone(),
        message: diag.message.clone(),
        entity_id: diag.entity_id.map(|id| id.to_string()),
    }
}

/// Maps a MergeProposal domain item to a MergeProposalDTO.
pub fn to_merge_proposal_dto(prop: &MergeProposal) -> MergeProposalDTO {
    MergeProposalDTO {
        source_entity_id: prop.source_entity_id.to_string(),
        target_entity_id: prop.target_entity_id.to_string(),
        confidence: prop.confidence,
        rationale: prop.rationale.clone(),
        feature_scores: prop.feature_scores.clone(),
    }
}

/// Maps a ContradictionRecord domain item to a ContradictionRecordDTO.
pub fn to_contradiction_record_dto(record: &ContradictionRecord) -> ContradictionRecordDTO {
    let kind = match record.kind {
        brain_domain::ContradictionKind::Logical => "logical",
        brain_domain::ContradictionKind::Temporal => "temporal",
    };
    ContradictionRecordDTO {
        kind: kind.to_string(),
        entity_a: record.entity_a.to_string(),
        entity_b: record.entity_b.to_string(),
        confidence: record.confidence,
        rationale: record.rationale.clone(),
    }
}

/// Maps a PassReport domain item to a PassReportDTO.
pub fn to_pass_report_dto(report: &PassReport) -> PassReportDTO {
    PassReportDTO {
        pass_name: report.pass_name.to_string(),
        items_processed: report.items_processed,
        changes_applied: report.changes_applied,
        diagnostics: report
            .diagnostics
            .iter()
            .map(to_pass_diagnostic_dto)
            .collect(),
        merge_proposals: report
            .merge_proposals
            .iter()
            .map(to_merge_proposal_dto)
            .collect(),
        contradiction_records: report
            .contradiction_records
            .iter()
            .map(to_contradiction_record_dto)
            .collect(),
        duration_ms: report.duration.as_millis() as u64,
    }
}

use crate::reflection::contracts::{ReflectionReport, TaskReport};
use brain_domain::{ReflectionReportDTO, TaskReportDTO};

/// Maps a TaskReport domain item to a TaskReportDTO.
pub fn to_task_report_dto(report: &TaskReport) -> TaskReportDTO {
    TaskReportDTO {
        task_name: report.task_name.to_string(),
        task_kind: format!("{:?}", report.task_kind).to_lowercase(),
        items_processed: report.items_processed,
        changes_applied: report.changes_applied,
        diagnostics: report
            .diagnostics
            .iter()
            .map(to_pass_diagnostic_dto)
            .collect(),
        duration_ms: report.duration.as_millis() as u64,
    }
}

/// Maps a ReflectionReport domain item to a ReflectionReportDTO.
pub fn to_reflection_report_dto(report: &ReflectionReport) -> ReflectionReportDTO {
    ReflectionReportDTO {
        execution_mode: format!("{:?}", report.execution_mode).to_lowercase(),
        task_reports: report.task_reports.iter().map(to_task_report_dto).collect(),
        total_duration_ms: report.total_duration.as_millis() as u64,
        total_changes: report.total_changes,
    }
}
