import type { BrainStreamChunk } from '../client/BrainBackendClient.js';
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
    case 'finished':
      return { type: 'turn_complete', stopReason: chunk.status };
    default:
      return null;
  }
}
