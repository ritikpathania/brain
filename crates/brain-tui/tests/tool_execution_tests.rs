use brain_tui::state::{UiState, Action, UpdateResult};
use brain_tui::ui::command::tool::{
    ToolCallId, ToolId, ToolExecutionStatus, ToolProgressDetail
};
use brain_core::events::ProgressUnit;

#[test]
fn test_tool_call_lifecycle_happy_path() {
    let mut state = UiState::new();
    let msg_id = brain_tui::ui::interaction::MessageId(42);
    let call_id = ToolCallId("call_1".to_string());
    let tool_id = ToolId("search_files".to_string());

    // 1. Request tool call that requires approval
    let res = state.update(Action::ToolCallRequested {
        message: msg_id,
        call_id: call_id.clone(),
        tool_id: tool_id.clone(),
        arguments: "{\"query\": \"rust\"}".to_string(),
        requires_approval: true,
    });
    assert!(matches!(res, UpdateResult::Changed));
    assert_eq!(state.active_tool_calls.len(), 1);
    assert_eq!(state.pending_approvals.len(), 1);
    assert_eq!(state.active_tool_calls[0].status, ToolExecutionStatus::PendingApproval);

    // 2. Try to request same tool call again (Idempotency)
    let res = state.update(Action::ToolCallRequested {
        message: msg_id,
        call_id: call_id.clone(),
        tool_id: tool_id.clone(),
        arguments: "{\"query\": \"rust\"}".to_string(),
        requires_approval: true,
    });
    assert!(matches!(res, UpdateResult::NoChange));
    assert_eq!(state.active_tool_calls.len(), 1);

    // 3. Approve tool call (FIFO)
    let res = state.update(Action::ApproveToolCall {
        call_id: call_id.clone(),
        approved: true,
    });
    assert!(matches!(res, UpdateResult::Changed));
    assert_eq!(state.pending_approvals.len(), 0);
    assert_eq!(state.active_tool_calls[0].status, ToolExecutionStatus::Approved);

    // 4. Progress update (Monotonicity & Logs)
    let res = state.update(Action::ToolProgressReceived {
        message: msg_id,
        call_id: call_id.clone(),
        sequence: 10,
        detail: ToolProgressDetail::Determinate {
            completed: 5,
            total: 10,
            unit: ProgressUnit::Items,
        },
        log_message: "Searching root directory".to_string(),
    });
    assert!(matches!(res, UpdateResult::Changed));
    assert_eq!(
        state.active_tool_calls[0].status,
        ToolExecutionStatus::Running {
            progress: ToolProgressDetail::Determinate {
                completed: 5,
                total: 10,
                unit: ProgressUnit::Items,
            }
        }
    );
    assert_eq!(state.active_tool_calls[0].logs.len(), 1);
    assert_eq!(state.active_tool_calls[0].logs[0].message, "Searching root directory");

    // 5. Old sequence progress received (should be ignored)
    let res = state.update(Action::ToolProgressReceived {
        message: msg_id,
        call_id: call_id.clone(),
        sequence: 9,
        detail: ToolProgressDetail::Indeterminate,
        log_message: "Stale progress logs".to_string(),
    });
    assert!(matches!(res, UpdateResult::NoChange));
    assert_eq!(
        state.active_tool_calls[0].status,
        ToolExecutionStatus::Running {
            progress: ToolProgressDetail::Determinate {
                completed: 5,
                total: 10,
                unit: ProgressUnit::Items,
            }
        }
    );

    // 6. Complete tool call -> moves to long-term owner
    let res = state.update(Action::ToolResultReceived {
        message: msg_id,
        call_id: call_id.clone(),
        result: "{\"found\": 4}".to_string(),
        is_error: false,
    });
    assert!(matches!(res, UpdateResult::Changed));
    assert_eq!(state.active_tool_calls.len(), 0);
    assert_eq!(state.message_tool_calls.len(), 1);
    let attached = &state.message_tool_calls[&msg_id];
    assert_eq!(attached.len(), 1);
    assert_eq!(attached[0].call_id, call_id);
    assert!(matches!(attached[0].status, ToolExecutionStatus::Completed { .. }));
}

#[test]
fn test_tool_call_denial() {
    let mut state = UiState::new();
    let msg_id = brain_tui::ui::interaction::MessageId(42);
    let call_id = ToolCallId("call_1".to_string());
    let tool_id = ToolId("search_files".to_string());

    state.update(Action::ToolCallRequested {
        message: msg_id,
        call_id: call_id.clone(),
        tool_id: tool_id.clone(),
        arguments: "{}".to_string(),
        requires_approval: true,
    });

    // Deny the approval -> terminal state and attaches to message
    let res = state.update(Action::ApproveToolCall {
        call_id: call_id.clone(),
        approved: false,
    });
    assert!(matches!(res, UpdateResult::Changed));
    assert_eq!(state.active_tool_calls.len(), 0);
    assert_eq!(state.message_tool_calls.len(), 1);
    let attached = &state.message_tool_calls[&msg_id];
    assert_eq!(attached[0].status, ToolExecutionStatus::Denied);
}
