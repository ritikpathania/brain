/**
 * Brain Turn Transformer (Pure Event Reducer)
 *
 * Deterministic reducer converting a stream or sequence of BrainTurnEvents
 * into an immutable BrainTurnViewModel.
 *
 * Invariants:
 * 1. Zero React, Ink, UDS, or transport dependencies.
 * 2. Deterministic: identical event sequences always produce deep-equal ViewModels.
 * 3. Safe: unknown events are safely ignored without corrupting state.
 * 4. Idempotent terminal events: multiple completion events do not corrupt VM.
 * 5. Local failure containment: tool or subagent failures do not tear down parent turn.
 */

import type { BrainTurnEvent } from './BrainTurnEvents.js';
import type {
  BrainTurnViewModel,
  ToolExecutionView,
  AgentExecutionView,
  MemoryProvenanceView,
} from './BrainViewModels.js';

export class BrainTurnTransformer {
  /**
   * Creates a pristine initial turn view model.
   */
  static createInitial(turnId: string = 'turn_init', role: 'user' | 'assistant' = 'assistant'): BrainTurnViewModel {
    return {
      id: turnId,
      role,
      content: '',
      status: 'streaming',
    };
  }

  /**
   * Pure reducer taking previous view model and applying a single domain event.
   */
  static reduce(state: BrainTurnViewModel, event: BrainTurnEvent): BrainTurnViewModel {
    switch (event.type) {
      case 'turn_start': {
        return {
          ...state,
          id: event.turnId || state.id,
          role: event.role || state.role,
        };
      }

      case 'thinking_start': {
        return {
          ...state,
          thinking: state.thinking
            ? { ...state.thinking, isComplete: false }
            : { text: '', isComplete: false },
        };
      }

      case 'thinking_delta': {
        const currentThinking = state.thinking || { text: '', isComplete: false };
        return {
          ...state,
          thinking: {
            ...currentThinking,
            text: currentThinking.text + event.delta,
          },
        };
      }

      case 'thinking_end': {
        if (!state.thinking) {
          return state;
        }
        return {
          ...state,
          thinking: {
            ...state.thinking,
            durationMs: event.durationMs !== undefined ? event.durationMs : state.thinking.durationMs,
            isComplete: true,
          },
        };
      }

      case 'text_delta': {
        return {
          ...state,
          content: state.content + event.delta,
        };
      }

      case 'memory_recalled': {
        const existing = state.memories || [];
        const existingIds = new Set(existing.map((m) => m.nodeId));
        const newAdditions: MemoryProvenanceView[] = [];

        for (const mem of event.memories) {
          if (!existingIds.has(mem.nodeId)) {
            existingIds.add(mem.nodeId);
            newAdditions.push(mem);
          }
        }

        const merged = [...existing, ...newAdditions];
        merged.sort((a, b) => b.confidence - a.confidence || a.label.localeCompare(b.label));

        return {
          ...state,
          memories: merged,
        };
      }

      case 'tool_call_requested': {
        const tools = state.tools ? [...state.tools] : [];
        const existingIdx = tools.findIndex((t) => t.callId === event.callId);

        const newTool: ToolExecutionView = {
          callId: event.callId,
          agentId: event.agentId,
          toolName: event.toolName,
          input: event.input,
          status: 'pending',
        };

        if (existingIdx >= 0) {
          tools[existingIdx] = { ...tools[existingIdx], ...newTool };
        } else {
          tools.push(newTool);
        }

        return {
          ...state,
          tools,
        };
      }

      case 'tool_permission_requested': {
        const tools = state.tools ? [...state.tools] : [];
        const idx = tools.findIndex((t) => t.callId === event.callId);

        const permTool: ToolExecutionView = {
          callId: event.callId,
          agentId: event.agentId,
          toolName: event.toolName,
          input: event.input,
          status: 'permission_required',
          permissionReason: event.reason,
        };

        if (idx >= 0) {
          tools[idx] = { ...tools[idx], ...permTool };
        } else {
          tools.push(permTool);
        }

        return { ...state, tools };
      }

      case 'tool_permission_resolved': {
        const tools = state.tools ? [...state.tools] : [];
        const idx = tools.findIndex((t) => t.callId === event.callId);

        if (idx >= 0) {
          tools[idx] = {
            ...tools[idx],
            status: event.granted ? 'pending' : 'denied',
            permissionReason: event.reason || tools[idx].permissionReason,
          };
          return { ...state, tools };
        }
        return state;
      }

      case 'tool_started': {
        const tools = state.tools ? [...state.tools] : [];
        const idx = tools.findIndex((t) => t.callId === event.callId);

        if (idx >= 0) {
          // If tool was previously denied, do NOT allow it to start
          if (tools[idx].status === 'denied') {
            return state;
          }
          tools[idx] = {
            ...tools[idx],
            status: 'running',
          };
          return { ...state, tools };
        }
        return state;
      }

      case 'tool_progress': {
        // Preserves tool progress state without mutating other fields
        return state;
      }

      case 'tool_result': {
        const tools = state.tools ? [...state.tools] : [];
        const idx = tools.findIndex((t) => t.callId === event.callId);
        const status = event.isError ? 'failed' : 'completed';

        if (idx >= 0) {
          // Terminal state idempotency: if already completed/failed with same output, keep
          tools[idx] = {
            ...tools[idx],
            output: event.output,
            isError: event.isError,
            // Inc 10: daemon-measured facts; a repeat result without them
            // never clobbers what was already recorded.
            durationMs: event.durationMs !== undefined ? event.durationMs : tools[idx].durationMs,
            exitCode: event.exitCode !== undefined ? event.exitCode : tools[idx].exitCode,
            status,
          };
        } else {
          tools.push({
            callId: event.callId,
            agentId: event.agentId,
            toolName: 'unknown_tool',
            input: {},
            output: event.output,
            isError: event.isError,
            durationMs: event.durationMs,
            exitCode: event.exitCode,
            status,
          });
        }

        return {
          ...state,
          tools,
        };
      }

      case 'tool_cancelled': {
        const tools = state.tools ? [...state.tools] : [];
        const idx = tools.findIndex((t) => t.callId === event.callId);

        if (idx >= 0) {
          tools[idx] = {
            ...tools[idx],
            status: 'cancelled',
          };
          return { ...state, tools };
        }
        return state;
      }

      case 'agent_started': {
        const agents = state.agents ? [...state.agents] : [];
        const idx = agents.findIndex((a) => a.agentId === event.agentId);

        const newAgent: AgentExecutionView = {
          agentId: event.agentId,
          role: event.role,
          status: 'executing',
        };

        if (idx >= 0) {
          agents[idx] = { ...agents[idx], ...newAgent };
        } else {
          agents.push(newAgent);
        }

        return {
          ...state,
          agents,
        };
      }

      case 'agent_progress': {
        const agents = state.agents ? [...state.agents] : [];
        const idx = agents.findIndex((a) => a.agentId === event.agentId);

        if (idx >= 0) {
          agents[idx] = {
            ...agents[idx],
            progressMessage: event.progressMessage,
          };
          return { ...state, agents };
        }
        return state;
      }

      case 'agent_completed': {
        const agents = state.agents ? [...state.agents] : [];
        const idx = agents.findIndex((a) => a.agentId === event.agentId);

        if (idx >= 0) {
          agents[idx] = {
            ...agents[idx],
            status: event.status,
          };
          return { ...state, agents };
        }
        return state;
      }

      case 'agent_failed': {
        const agents = state.agents ? [...state.agents] : [];
        const idx = agents.findIndex((a) => a.agentId === event.agentId);

        if (idx >= 0) {
          agents[idx] = {
            ...agents[idx],
            status: 'failed',
            error: event.error,
          };
          return { ...state, agents };
        }
        return state;
      }

      case 'agent_cancelled': {
        const agents = state.agents ? [...state.agents] : [];
        const idx = agents.findIndex((a) => a.agentId === event.agentId);

        if (idx >= 0) {
          agents[idx] = {
            ...agents[idx],
            status: 'cancelled',
          };
          return { ...state, agents };
        }
        return state;
      }

      case 'turn_complete': {
        return {
          ...state,
          status: 'completed',
          durationMs: event.durationMs !== undefined ? event.durationMs : state.durationMs,
        };
      }

      case 'turn_error': {
        return {
          ...state,
          status: 'error',
          error: event.error,
        };
      }

      case 'unknown':
      default:
        // Graceful forward compatibility: ignore unrecognized events
        return state;
    }
  }

  /**
   * Replays an entire collection of events deterministically into a finished view model.
   */
  static transform(events: Iterable<BrainTurnEvent>, initialState?: BrainTurnViewModel): BrainTurnViewModel {
    let current = initialState || BrainTurnTransformer.createInitial();
    for (const event of events) {
      current = BrainTurnTransformer.reduce(current, event);
    }
    return current;
  }
}
