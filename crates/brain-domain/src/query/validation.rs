use crate::validation::{AffectedElement, ValidationDiagnostic, ValidationReport};

/// Service querying diagnostics reports for specific elements.
pub struct ValidationQueryService;

impl ValidationQueryService {
    /// Finds diagnostics that affect the target element.
    pub fn find_diagnostics_for_element<'a>(
        element: &AffectedElement,
        report: &'a ValidationReport,
    ) -> Vec<&'a ValidationDiagnostic> {
        report
            .diagnostics
            .iter()
            .filter(|d| d.affected.contains(element))
            .collect()
    }
}
