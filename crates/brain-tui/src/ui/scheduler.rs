//! Render scheduling and interface invalidation models.

/// Scope of UI components that became stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderInvalidation {
    /// Conversation message log panel is stale.
    ConversationStale,
    /// Prompt text editor input box is stale.
    EditorStale,
    /// Connection state/status bar header is stale.
    StatusBarStale,
    /// Full screen redraw is required.
    EverythingStale,
}

/// Category of UI changes triggering redraws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderReason {
    /// User keyboard character input or editing operation.
    Input,
    /// Stream chunk token received from daemon.
    StreamToken,
    /// Terminal dimensions window resize event.
    Resize,
    /// Theme change request.
    ThemeChanged,
    /// Focus context focus traversal swap.
    FocusChanged,
}

/// Request bundle submitted to the RenderScheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderRequest {
    /// Specific trigger reason.
    pub reason: RenderReason,
    /// Specific invalidation scope.
    pub invalidation: RenderInvalidation,
}

impl RenderRequest {
    /// Pure functional merge returning the coalesced request.
    pub fn coalesce(self, other: RenderRequest) -> RenderRequest {
        // Upgrade reason to higher priority if needed
        let reason = match (self.reason, other.reason) {
            (RenderReason::Resize, _) | (_, RenderReason::Resize) => RenderReason::Resize,
            (RenderReason::ThemeChanged, _) | (_, RenderReason::ThemeChanged) => RenderReason::ThemeChanged,
            (RenderReason::FocusChanged, _) | (_, RenderReason::FocusChanged) => RenderReason::FocusChanged,
            (RenderReason::Input, _) | (_, RenderReason::Input) => RenderReason::Input,
            _ => RenderReason::StreamToken,
        };

        // Coalesce invalidation scopes
        let invalidation = match (self.invalidation, other.invalidation) {
            (RenderInvalidation::EverythingStale, _) | (_, RenderInvalidation::EverythingStale) => {
                RenderInvalidation::EverythingStale
            }
            (RenderInvalidation::ConversationStale, RenderInvalidation::StatusBarStale)
            | (RenderInvalidation::StatusBarStale, RenderInvalidation::ConversationStale) => {
                RenderInvalidation::EverythingStale
            }
            (RenderInvalidation::ConversationStale, RenderInvalidation::EditorStale)
            | (RenderInvalidation::EditorStale, RenderInvalidation::ConversationStale) => {
                RenderInvalidation::EverythingStale
            }
            (RenderInvalidation::StatusBarStale, RenderInvalidation::EditorStale)
            | (RenderInvalidation::EditorStale, RenderInvalidation::StatusBarStale) => {
                RenderInvalidation::EverythingStale
            }
            (a, _) => a,
        };

        RenderRequest { reason, invalidation }
    }
}

/// Abstract interface for frame scheduling optimization.
pub trait RenderScheduler {
    /// Submits a request to repaint the interface.
    fn request(&self, request: RenderRequest);
}

use std::sync::Mutex;

/// A thread-safe mock implementation of RenderScheduler for testing and verification.
#[derive(Debug, Default)]
pub struct MockRenderScheduler {
    requests: Mutex<Vec<RenderRequest>>,
}

impl MockRenderScheduler {
    /// Instantiates a new MockRenderScheduler.
    pub fn new() -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Access the captured requests.
    pub fn requests(&self) -> Vec<RenderRequest> {
        self.requests.lock().unwrap().clone()
    }

    /// Clears the captured requests.
    pub fn clear(&self) {
        self.requests.lock().unwrap().clear();
    }
}

impl RenderScheduler for MockRenderScheduler {
    fn request(&self, request: RenderRequest) {
        self.requests.lock().unwrap().push(request);
    }
}
