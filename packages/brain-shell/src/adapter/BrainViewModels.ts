/**
 * Brain Capability-Neutral Presentation View Models
 *
 * Defines the presentation contract consumed by rendering components.
 * Contains zero transport, UDS, React, or database dependencies.
 */

export interface MemoryRelationView {
  targetId: string;
  relation: string;
  targetLabel?: string;
}

export interface MemoryProvenanceView {
  nodeId: string;
  label: string;
  score: number;
  confidence?: number;
  source: string;
  excerpt?: string;
  relations?: MemoryRelationView[];
}

export interface ToolExecutionView {
  callId: string;
  agentId?: string;
  toolName: string;
  input: Record<string, unknown>;
  output?: string;
  isError?: boolean;
  status: 'pending' | 'permission_required' | 'running' | 'completed' | 'failed' | 'denied' | 'cancelled';
  permissionReason?: string;
  durationMs?: number;
  /** Process exit code from the daemon (Inc 10); rendered on failed cards. */
  exitCode?: number;
}

export interface AgentExecutionView {
  agentId: string;
  role: string;
  status: 'planning' | 'executing' | 'idle' | 'completed' | 'failed' | 'cancelled';
  progressMessage?: string;
  error?: string;
  durationMs?: number;
}

export interface BrainThinkingViewModel {
  text: string;
  durationMs?: number;
  isComplete: boolean;
}

export interface BrainTurnViewModel {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  status: 'streaming' | 'completed' | 'error';
  thinking?: BrainThinkingViewModel;
  memories?: MemoryProvenanceView[];
  tools?: ToolExecutionView[];
  agents?: AgentExecutionView[];
  error?: string;
  durationMs?: number;
}

export interface DiagnosticSubsystemView {
  name: string;
  status: 'healthy' | 'degraded' | 'unhealthy';
  message: string;
  latencyMs?: number;
  metrics?: Record<string, string | number | boolean>;
}

export interface BrainDoctorViewModel {
  timestamp: string;
  isOverallHealthy: boolean;
  version: string;
  subsystems: DiagnosticSubsystemView[];
  summary: string;
}
