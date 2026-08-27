/**
 * Inc 36: Unit tests for context retrieval provenance preservation across
 * BrainStreamChunk -> chunkToTurnEvent -> BrainTurnEvent -> BrainTurnTransformer -> turnToRows.
 */
import { describe, it, expect } from 'bun:test';
import { chunkToTurnEvent } from '../../adapter/chunkToTurnEvents.js';
import { BrainTurnTransformer } from '../../adapter/BrainTurnTransformer.js';
import { turnToRows } from '../../ui/transcript/toRows.js';
import type { BrainStreamChunk, ContextTelemetry } from '../../client/BrainBackendClient.js';
import type { BrainTurnEvent } from '../../adapter/BrainTurnEvents.js';

describe('Context Provenance Wire Transformation', () => {
  const SAMPLE_TELEMETRY = {
    generation_id: 'gen-12345',
    session_id: '01M1234567890ABCDEF',
    retrieval_epoch_id: 'ep-epoch-999',
    candidates_retrieved: 8,
    memories_assembled: 3,
    context_tokens_used: 240,
    assembly_latency_ms: 12,
    total_duration_ms: 450,
    finish_reason: 'stop',
  };

  it('chunkToTurnEvent preserves telemetry faithfully from finished chunk', () => {
    const chunk: BrainStreamChunk = {
      type: 'finished',
      generationId: 'gen-12345',
      sessionId: '01M1234567890ABCDEF',
      sequence: 5,
      status: 'completed',
      metadata: {
        telemetry: SAMPLE_TELEMETRY,
      },
    };

    const event = chunkToTurnEvent(chunk);
    expect(event).not.toBeNull();
    expect(event!.type).toBe('turn_complete');
    if (event && event.type === 'turn_complete') {
      expect(event.telemetry).toEqual({
        generationId: 'gen-12345',
        sessionId: '01M1234567890ABCDEF',
        retrievalEpochId: 'ep-epoch-999',
        candidatesRetrieved: 8,
        memoriesAssembled: 3,
        contextTokensUsed: 240,
        assemblyLatencyMs: 12,
        totalDurationMs: 450,
        finishReason: 'stop',
      });
    }
  });

  it('chunkToTurnEvent handles missing telemetry without defaulting to manufactured values', () => {
    const chunk: BrainStreamChunk = {
      type: 'finished',
      generationId: 'gen-plain',
      status: 'completed',
    };

    const event = chunkToTurnEvent(chunk);
    expect(event).not.toBeNull();
    if (event && event.type === 'turn_complete') {
      expect(event.telemetry).toBeUndefined();
    }
  });

  it('BrainTurnTransformer.transform reduces telemetry into BrainTurnViewModel without mutation', () => {
    const telemetry: ContextTelemetry = {
      generationId: 'gen-12345',
      sessionId: '01M1234567890ABCDEF',
      retrievalEpochId: 'ep-epoch-999',
      candidatesRetrieved: 5,
      memoriesAssembled: 2,
      contextTokensUsed: 180,
      assemblyLatencyMs: 8,
      totalDurationMs: 300,
      finishReason: 'stop',
    };

    const events: BrainTurnEvent[] = [
      { type: 'turn_start', turnId: 'turn-1', role: 'assistant' },
      { type: 'text_delta', delta: 'Quantum processors require dilution cooling.' },
      { type: 'turn_complete', telemetry },
    ];

    const vm = BrainTurnTransformer.transform(events);
    expect(vm.status).toBe('completed');
    expect(vm.content).toBe('Quantum processors require dilution cooling.');
    expect(vm.telemetry).toEqual(telemetry);
  });

  it('turnToRows emits a context row and attaches telemetry when memoriesAssembled > 0', () => {
    const telemetry: ContextTelemetry = {
      generationId: 'gen-12345',
      sessionId: '01M1234567890ABCDEF',
      retrievalEpochId: 'ep-epoch-999',
      candidatesRetrieved: 6,
      memoriesAssembled: 2,
      contextTokensUsed: 150,
      assemblyLatencyMs: 5,
      totalDurationMs: 200,
    };

    const vm = {
      id: 'turn-1',
      role: 'assistant' as const,
      content: 'Here is the cryogenic analysis.',
      status: 'completed' as const,
      telemetry,
    };

    const rows = turnToRows(vm);
    expect(rows.length).toBe(2);

    const assistantRow = rows.find((r) => r.kind === 'assistant');
    expect(assistantRow).toBeDefined();
    if (assistantRow && assistantRow.kind === 'assistant') {
      expect(assistantRow.telemetry).toEqual(telemetry);
    }

    const contextRow = rows.find((r) => r.kind === 'context');
    expect(contextRow).toBeDefined();
    if (contextRow && contextRow.kind === 'context') {
      expect(contextRow.telemetry).toEqual(telemetry);
    }
  });

  it('turnToRows omits context row when memoriesAssembled === 0', () => {
    const telemetry: ContextTelemetry = {
      generationId: 'gen-12345',
      sessionId: '01M1234567890ABCDEF',
      retrievalEpochId: 'ep-epoch-000',
      candidatesRetrieved: 3,
      memoriesAssembled: 0,
      contextTokensUsed: 0,
      assemblyLatencyMs: 4,
      totalDurationMs: 150,
    };

    const vm = {
      id: 'turn-2',
      role: 'assistant' as const,
      content: 'No memories augmented.',
      status: 'completed' as const,
      telemetry,
    };

    const rows = turnToRows(vm);
    expect(rows.length).toBe(1);
    expect(rows[0].kind).toBe('assistant');
    if (rows[0].kind === 'assistant') {
      expect(rows[0].telemetry).toEqual(telemetry);
    }
    expect(rows.some((r) => r.kind === 'context')).toBe(false);
  });
});
