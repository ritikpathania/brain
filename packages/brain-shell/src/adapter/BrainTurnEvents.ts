/**
 * Brain Application Turn Events
 *
 * Defines transport-independent domain execution events consumed by presentation adapters.
 * Decouples transport protocol (UDS, in-process, IPC) from presentation view model generation.
 */

import type { MemoryProvenanceView } from './BrainViewModels.js';

export type BrainTurnEvent =
  | { type: 'turn_start'; turnId: string; role: 'user' | 'assistant' }
  | { type: 'thinking_start' }
  | { type: 'thinking_delta'; delta: string }
  | { type: 'thinking_end'; durationMs?: number }
  | { type: 'text_delta'; delta: string }
  | { type: 'memory_recalled'; memories: MemoryProvenanceView[] }
  | { type: 'tool_call_requested'; callId: string; toolName: string; input: Record<string, unknown>; agentId?: string }
  | { type: 'tool_permission_requested'; callId: string; toolName: string; input: Record<string, unknown>; reason?: string; agentId?: string }
  | { type: 'tool_permission_resolved'; callId: string; granted: boolean; reason?: string; agentId?: string }
  | { type: 'tool_started'; callId: string; agentId?: string }
  | { type: 'tool_progress'; callId: string; progressMessage: string; agentId?: string }
  | { type: 'tool_result'; callId: string; output: string; isError?: boolean; exitCode?: number; durationMs?: number; agentId?: string }
  | { type: 'tool_cancelled'; callId: string; reason?: string; agentId?: string }
  | { type: 'agent_started'; agentId: string; role: string }
  | { type: 'agent_progress'; agentId: string; progressMessage: string }
  | { type: 'agent_completed'; agentId: string; status: 'completed' | 'failed' }
  | { type: 'agent_failed'; agentId: string; error: string }
  | { type: 'agent_cancelled'; agentId: string; reason?: string }
  | { type: 'turn_complete'; durationMs?: number; stopReason?: string }
  | { type: 'turn_error'; error: string }
  | { type: 'unknown'; rawType: string; payload?: unknown };
