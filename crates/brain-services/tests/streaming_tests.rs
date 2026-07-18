use brain_core::extensibility::CancellationToken;
use brain_services::agent::streaming::{
    DefaultStreamEventMapper, OverflowPolicy, ProgressEvent, SafeEventQueue, StreamEvent,
    StreamEventPayload, StreamingRuntime, TokenEvent,
};
use brain_services::agent::{AgentExecutionEvent, AgentExecutionEventPayload, ExecutionId};
use brain_tools::CancellationTokenImpl;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

#[tokio::test]
async fn test_selective_drop_backpressure() {
    // Bounded queue with capacity 3, using SelectiveDrop policy
    let queue = SafeEventQueue::new(3, OverflowPolicy::SelectiveDrop);
    let execution_id = ExecutionId::new();

    let p1 = StreamEvent {
        execution_id,
        sequence: 1,
        timestamp: SystemTime::now(),
        payload: StreamEventPayload::Progress(ProgressEvent {
            message: "P1".to_string(),
            percentage: None,
        }),
    };
    let t1 = StreamEvent {
        execution_id,
        sequence: 2,
        timestamp: SystemTime::now(),
        payload: StreamEventPayload::Token(TokenEvent {
            token: "T1".to_string(),
        }),
    };
    let p2 = StreamEvent {
        execution_id,
        sequence: 3,
        timestamp: SystemTime::now(),
        payload: StreamEventPayload::Progress(ProgressEvent {
            message: "P2".to_string(),
            percentage: None,
        }),
    };

    queue.push(p1);
    queue.push(t1);
    queue.push(p2);

    // Queue is full at capacity 3: [P1, T1, P2]
    // Push another progress event P3.
    // It should drop the oldest non-essential event (P1) and push P3.
    let p3 = StreamEvent {
        execution_id,
        sequence: 4,
        timestamp: SystemTime::now(),
        payload: StreamEventPayload::Progress(ProgressEvent {
            message: "P3".to_string(),
            percentage: None,
        }),
    };
    queue.push(p3);

    // Expecting: [T1, P2, P3]
    // Let's push an essential token event T2.
    // It should drop the oldest non-essential event in the queue (P2) and push T2.
    let t2 = StreamEvent {
        execution_id,
        sequence: 5,
        timestamp: SystemTime::now(),
        payload: StreamEventPayload::Token(TokenEvent {
            token: "T2".to_string(),
        }),
    };
    queue.push(t2);

    // Expecting: [T1, P3, T2]
    // Push another essential token T3.
    // It should drop the remaining non-essential event (P3) and push T3.
    let t3 = StreamEvent {
        execution_id,
        sequence: 6,
        timestamp: SystemTime::now(),
        payload: StreamEventPayload::Token(TokenEvent {
            token: "T3".to_string(),
        }),
    };
    queue.push(t3);

    // Expecting: [T1, T2, T3] (fully essential events)
    // Push another essential event T4.
    // Since there are only essential events in the queue, it exceeds the limit (soft-limit) and pushes T4.
    let t4 = StreamEvent {
        execution_id,
        sequence: 7,
        timestamp: SystemTime::now(),
        payload: StreamEventPayload::Token(TokenEvent {
            token: "T4".to_string(),
        }),
    };
    queue.push(t4);

    queue.close();

    // Verify queue items
    let mut items = Vec::new();
    let stream =
        brain_services::agent::streaming::ExecutionStream::new_test(Arc::new(queue), execution_id);

    while let Some(evt) = stream.next().await {
        items.push(evt);
    }

    assert_eq!(items.len(), 4);

    // T1
    if let StreamEventPayload::Token(t) = &items[0].payload {
        assert_eq!(t.token, "T1");
    } else {
        panic!("Expected T1");
    }

    // T2
    if let StreamEventPayload::Token(t) = &items[1].payload {
        assert_eq!(t.token, "T2");
    } else {
        panic!("Expected T2");
    }

    // T3
    if let StreamEventPayload::Token(t) = &items[2].payload {
        assert_eq!(t.token, "T3");
    } else {
        panic!("Expected T3");
    }

    // T4
    if let StreamEventPayload::Token(t) = &items[3].payload {
        assert_eq!(t.token, "T4");
    } else {
        panic!("Expected T4");
    }
}

#[tokio::test]
async fn test_drop_oldest_overflow() {
    let queue = SafeEventQueue::new(2, OverflowPolicy::DropOldest);
    let execution_id = ExecutionId::new();

    let e1 = StreamEvent {
        execution_id,
        sequence: 1,
        timestamp: SystemTime::now(),
        payload: StreamEventPayload::Progress(ProgressEvent {
            message: "E1".to_string(),
            percentage: None,
        }),
    };
    let e2 = StreamEvent {
        execution_id,
        sequence: 2,
        timestamp: SystemTime::now(),
        payload: StreamEventPayload::Progress(ProgressEvent {
            message: "E2".to_string(),
            percentage: None,
        }),
    };
    let e3 = StreamEvent {
        execution_id,
        sequence: 3,
        timestamp: SystemTime::now(),
        payload: StreamEventPayload::Progress(ProgressEvent {
            message: "E3".to_string(),
            percentage: None,
        }),
    };

    queue.push(e1);
    queue.push(e2);
    queue.push(e3); // Drops E1, keeps [E2, E3]
    queue.close();

    let mut items = Vec::new();
    let stream =
        brain_services::agent::streaming::ExecutionStream::new_test(Arc::new(queue), execution_id);
    while let Some(evt) = stream.next().await {
        items.push(evt);
    }

    assert_eq!(items.len(), 2);
    if let StreamEventPayload::Progress(p) = &items[0].payload {
        assert_eq!(p.message, "E2");
    } else {
        panic!("Expected E2");
    }
    if let StreamEventPayload::Progress(p) = &items[1].payload {
        assert_eq!(p.message, "E3");
    } else {
        panic!("Expected E3");
    }
}

#[tokio::test]
async fn test_drop_newest_overflow() {
    let queue = SafeEventQueue::new(2, OverflowPolicy::DropNewest);
    let execution_id = ExecutionId::new();

    let e1 = StreamEvent {
        execution_id,
        sequence: 1,
        timestamp: SystemTime::now(),
        payload: StreamEventPayload::Progress(ProgressEvent {
            message: "E1".to_string(),
            percentage: None,
        }),
    };
    let e2 = StreamEvent {
        execution_id,
        sequence: 2,
        timestamp: SystemTime::now(),
        payload: StreamEventPayload::Progress(ProgressEvent {
            message: "E2".to_string(),
            percentage: None,
        }),
    };
    let e3 = StreamEvent {
        execution_id,
        sequence: 3,
        timestamp: SystemTime::now(),
        payload: StreamEventPayload::Progress(ProgressEvent {
            message: "E3".to_string(),
            percentage: None,
        }),
    };

    queue.push(e1);
    queue.push(e2);
    queue.push(e3); // Drops E3, keeps [E1, E2]
    queue.close();

    let mut items = Vec::new();
    let stream =
        brain_services::agent::streaming::ExecutionStream::new_test(Arc::new(queue), execution_id);
    while let Some(evt) = stream.next().await {
        items.push(evt);
    }

    assert_eq!(items.len(), 2);
    if let StreamEventPayload::Progress(p) = &items[0].payload {
        assert_eq!(p.message, "E1");
    } else {
        panic!("Expected E1");
    }
    if let StreamEventPayload::Progress(p) = &items[1].payload {
        assert_eq!(p.message, "E2");
    } else {
        panic!("Expected E2");
    }
}

#[tokio::test]
async fn test_streaming_runtime_lifecycle() {
    let mapper = Arc::new(DefaultStreamEventMapper);
    let runtime = StreamingRuntime::new(mapper);
    let execution_id = ExecutionId::new();
    let cancellation = Arc::new(CancellationTokenImpl::new());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    // Register active execution
    runtime.register(execution_id, rx, cancellation.clone());

    // Subscribe to execution
    let stream = runtime
        .subscribe(execution_id)
        .expect("Should subscribe successfully");

    // Emit events
    let t_start = SystemTime::now();
    tx.send(AgentExecutionEvent {
        execution_id,
        sequence: 1,
        timestamp: t_start,
        payload: AgentExecutionEventPayload::ExecutionStarted {
            session_id: brain_domain::SessionId::new(),
            prompt: "ping".to_string(),
        },
    })
    .unwrap();

    tx.send(AgentExecutionEvent {
        execution_id,
        sequence: 2,
        timestamp: t_start + Duration::from_millis(5),
        payload: AgentExecutionEventPayload::StageStarted {
            session_id: brain_domain::SessionId::new(),
            stage: "Planning",
        },
    })
    .unwrap();

    tx.send(AgentExecutionEvent {
        execution_id,
        sequence: 3,
        timestamp: t_start + Duration::from_millis(25),
        payload: AgentExecutionEventPayload::StageCompleted {
            session_id: brain_domain::SessionId::new(),
            stage: "Planning",
            duration_ms: 20,
        },
    })
    .unwrap();

    tx.send(AgentExecutionEvent {
        execution_id,
        sequence: 4,
        timestamp: t_start + Duration::from_millis(30),
        payload: AgentExecutionEventPayload::TokenStreamed {
            session_id: brain_domain::SessionId::new(),
            token: "hello world".to_string(),
        },
    })
    .unwrap();

    tx.send(AgentExecutionEvent {
        execution_id,
        sequence: 5,
        timestamp: t_start + Duration::from_millis(35),
        payload: AgentExecutionEventPayload::ExecutionFinished {
            session_id: brain_domain::SessionId::new(),
            response: "done".to_string(),
        },
    })
    .unwrap();

    // Close sender to complete the stream pipeline
    drop(tx);

    let mut events = Vec::new();
    while let Some(evt) = stream.next().await {
        events.push(evt);
    }

    // Checking sequence and mapped types
    assert!(events.len() >= 5);

    // Verify timeline builder snapshot inside the streamed events
    let timeline_event = events
        .iter()
        .find(|e| matches!(e.payload, StreamEventPayload::Timeline(_)))
        .expect("Should contain a timeline event");
    if let StreamEventPayload::Timeline(t) = &timeline_event.payload {
        assert_eq!(t.entry.stage, "Planning");
        assert_eq!(t.entry.duration, Duration::from_millis(20));
    }

    // Verify metrics collector snapshot
    // Since FinishedEvent was mapped, it injected the metrics snapshot
    let finished_opt = events
        .iter()
        .find(|e| matches!(e.payload, StreamEventPayload::Finished(_)));
    assert!(finished_opt.is_some());
    if let StreamEventPayload::Finished(f) = &finished_opt.unwrap().payload {
        assert_eq!(f.metrics.tokens_used, 2); // "hello", "world"
        assert_eq!(f.metrics.step_count, 1); // Planning stage completed
    }
}

#[tokio::test]
async fn test_subscriber_joins_mid_execution() {
    let mapper = Arc::new(DefaultStreamEventMapper);
    let runtime = StreamingRuntime::new(mapper);
    let execution_id = ExecutionId::new();
    let cancellation = Arc::new(CancellationTokenImpl::new());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    runtime.register(execution_id, rx, cancellation.clone());

    tx.send(AgentExecutionEvent {
        execution_id,
        sequence: 1,
        timestamp: SystemTime::now(),
        payload: AgentExecutionEventPayload::ExecutionStarted {
            session_id: brain_domain::SessionId::new(),
            prompt: "first".to_string(),
        },
    })
    .unwrap();

    // Briefly sleep to let first event process
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Join mid-execution
    let stream = runtime.subscribe(execution_id).expect("Should subscribe");

    tx.send(AgentExecutionEvent {
        execution_id,
        sequence: 2,
        timestamp: SystemTime::now(),
        payload: AgentExecutionEventPayload::TokenStreamed {
            session_id: brain_domain::SessionId::new(),
            token: "second".to_string(),
        },
    })
    .unwrap();

    drop(tx);

    let mut events = Vec::new();
    while let Some(evt) = stream.next().await {
        events.push(evt);
    }

    // Mid-subscriber should receive the replayed first event and the dynamic second event
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].sequence, 1);
    assert_eq!(events[1].sequence, 2);
}

#[tokio::test]
async fn test_cancellation_propagation() {
    let mapper = Arc::new(DefaultStreamEventMapper);
    let runtime = StreamingRuntime::new(mapper);
    let execution_id = ExecutionId::new();
    let cancellation = Arc::new(CancellationTokenImpl::new());
    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();

    runtime.register(execution_id, rx, cancellation.clone());

    assert!(!cancellation.is_cancelled());

    // Cancel through runtime
    let success = runtime.cancel(execution_id);
    assert!(success);
    assert!(cancellation.is_cancelled());
}
