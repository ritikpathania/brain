import type { BrainStreamChunk, ContextTelemetry } from '../client/BrainBackendClient.js';
import type { BrainTurnEvent } from './BrainTurnEvents.js';

/**
 * Pure projection: transport chunk → presentation turn event.
 * Unknown chunk shapes return null (renderer never crashes on bad frames).
 */
export function chunkToTurnEvent(chunk: BrainStreamChunk): BrainTurnEvent | null {
  switch (chunk.type) {
    case 'token':
      return typeof chunk.token === 'string' && chunk.token.length > 0
        ? { type: 'text_delta', delta: chunk.token }
        : null;
    case 'thinking':
      return typeof chunk.thinking === 'string' && chunk.thinking.length > 0
        ? { type: 'thinking_delta', delta: chunk.thinking }
        : null;
    case 'redacted_thinking':
      return { type: 'thinking_delta', delta: '[redacted thinking]' };
    case 'thinking_start':
      return { type: 'thinking_start' };
    case 'thinking_end':
      // Inc 13: daemon-measured phase duration rides along when the wire
      // carries it; the controller supplies a fallback otherwise.
      return {
        type: 'thinking_end',
        durationMs: typeof chunk.durationMs === 'number' ? chunk.durationMs : undefined,
      };
    case 'tool_use':
      return chunk.toolUse
        ? {
            type: 'tool_call_requested',
            callId: chunk.toolUse.id,
            toolName: chunk.toolUse.name,
            input: chunk.toolUse.input ?? {},
          }
        : null;
    case 'tool_result':
      return typeof chunk.callId === 'string'
        ? {
            type: 'tool_result',
            callId: chunk.callId,
            output: typeof chunk.output === 'string' ? chunk.output : '',
            isError: chunk.isError === true ? true : undefined,
            // Inc 10: daemon-measured execution facts ride along so live
            // cards match their persisted replay exactly.
            exitCode: typeof chunk.exitCode === 'number' ? chunk.exitCode : undefined,
            durationMs: typeof chunk.durationMs === 'number' ? chunk.durationMs : undefined,
          }
        : null;
    case 'error':
      return { type: 'turn_error', error: chunk.error ?? 'Unknown daemon error' };
    case 'finished': {
      let telemetry: ContextTelemetry | undefined = undefined;
      const rawTelem = chunk.metadata?.telemetry;
      if (rawTelem && typeof rawTelem === 'object') {
        telemetry = {
          generationId: typeof rawTelem.generation_id === 'string' ? rawTelem.generation_id : chunk.generationId,
          sessionId: typeof rawTelem.session_id === 'string' ? rawTelem.session_id : chunk.sessionId,
          retrievalEpochId: typeof rawTelem.retrieval_epoch_id === 'string' ? rawTelem.retrieval_epoch_id : undefined,
          candidatesRetrieved: typeof rawTelem.candidates_retrieved === 'number' ? rawTelem.candidates_retrieved : 0,
          memoriesAssembled: typeof rawTelem.memories_assembled === 'number' ? rawTelem.memories_assembled : 0,
          contextTokensUsed: typeof rawTelem.context_tokens_used === 'number' ? rawTelem.context_tokens_used : 0,
          assemblyLatencyMs: typeof rawTelem.assembly_latency_ms === 'number' ? rawTelem.assembly_latency_ms : undefined,
          totalDurationMs: typeof rawTelem.total_duration_ms === 'number' ? rawTelem.total_duration_ms : undefined,
          finishReason: typeof rawTelem.finish_reason === 'string' ? rawTelem.finish_reason : chunk.status,
        };
      }
      return {
        type: 'turn_complete',
        stopReason: chunk.status,
        durationMs: typeof chunk.durationMs === 'number' ? chunk.durationMs : telemetry?.totalDurationMs,
        telemetry,
      };
    }
    default:
      return null;
  }
}
