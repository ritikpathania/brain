/**
 * Transport-Agnostic Brain Backend Client Interface (Phase 5.5 — Thinking Support)
 *
 * Defines the clean boundary between the TypeScript CallModel adapter
 * and Brain backend execution runtimes, including reasoning / thinking blocks.
 */

export interface BrainToolDefinition {
  name: string;
  description: string;
  inputSchema?: Record<string, unknown>;
}

export interface BrainToolUseBlock {
  type: 'tool_use';
  id: string;
  name: string;
  input: Record<string, unknown>;
}

export interface BrainToolResultBlock {
  type: 'tool_result';
  tool_use_id: string;
  content: string;
  is_error?: boolean;
}

export interface BrainTextBlock {
  type: 'text';
  text: string;
}

export interface BrainThinkingBlock {
  type: 'thinking';
  thinking: string;
  signature?: string;
}

export interface BrainRedactedThinkingBlock {
  type: 'redacted_thinking';
  data: string;
}

export type BrainContentBlock =
  | BrainTextBlock
  | BrainThinkingBlock
  | BrainRedactedThinkingBlock
  | BrainToolUseBlock
  | BrainToolResultBlock;

export interface BrainChatMessage {
  role: 'user' | 'assistant' | 'system';
  content: string | BrainContentBlock[];
}

export interface BrainThinkingConfig {
  mode: 'adaptive' | 'enabled' | 'disabled';
  budgetTokens?: number;
}

export interface BrainGenerationRequest {
  sessionId?: string;
  generationId?: string;
  messages: BrainChatMessage[];
  systemPrompt?: string;
  tools?: BrainToolDefinition[];
  thinkingConfig?: BrainThinkingConfig;
  model?: string;
  signal?: AbortSignal;
}

export interface BrainStreamChunk {
  type:
    | 'token'
    | 'thinking'
    | 'redacted_thinking'
    | 'tool_use'
    | 'error'
    | 'finished'
    | 'permission_request'
    | 'tool_result';
  token?: string;
  thinking?: string;
  signature?: string;
  redactedData?: string;
  toolUse?: {
    id: string;
    name: string;
    input: Record<string, unknown>;
  };
  /** Present when type === 'permission_request'. */
  callId?: string;
  toolName?: string;
  input?: Record<string, unknown>;
  reason?: string;
  /** Present when type === 'tool_result'. */
  output?: string;
  isError?: boolean;
  exitCode?: number;
  error?: string;
  generationId?: string;
  sessionId?: string;
  sequence?: number;
  status?: 'in_progress' | 'completed' | 'cancelled' | 'failed';
  metadata?: {
    model?: string;
    inputTokens?: number;
    outputTokens?: number;
    memory_provenance?: MemoryProvenance;
  };
}

export interface MemoryProvenance {
  count: number;
  sources: string[];
  channels: string[];
  min_score?: number;
  max_score?: number;
  epoch_id?: string;
}

export interface MemoryRelation {
  target_id: string;
  relation: string;
  target_label?: string;
}

export interface RetrievedMemory {
  node_id: string;
  label: string;
  excerpt: string;
  channel: string;
  score: number;
  timestamp: number;
  scope: string;
  relations?: MemoryRelation[];
}

export interface ContextBudget {
  maxEstimatedTokens: number;
  maxCharacters?: number;
}

export interface CompiledMemoryItem {
  nodeId: string;
  label: string;
  excerpt: string;
  channel: string;
  score: number;
  scope: string;
  relations: MemoryRelation[];
  estimatedTokens: number;
}

export interface CompiledContextProvenance {
  count: number;
  sources: string[];
  channels: string[];
  epochId?: string;
  truncated: boolean;
  totalEstimatedTokens: number;
  budget: ContextBudget;
}

export interface CompiledContext {
  serializedPromptSection: string;
  memories: CompiledMemoryItem[];
  provenance: CompiledContextProvenance;
  hasContext: boolean;
}

export interface ContextRetrieveRequest {
  session_id: string;
  query: string;
  workspace_id?: string;
  limit?: number;
  max_tokens?: number;
}

export interface ContextRetrieveResponse {
  memories: RetrievedMemory[];
  provenance: MemoryProvenance;
  token_count: number;
  serialized_context: string;
}

export interface BrainSessionSummary {
  id: string;
  title: string;
  updatedAtMs: number;
  pinned: boolean;
  archived: boolean;
}

export interface BrainMessage {
  id: string;
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: string;
  timestampMs?: number;
}

export interface BrainSession {
  id: string;
  title: string;
  createdAtMs: number;
  updatedAtMs: number;
  pinned: boolean;
  archived: boolean;
  messages: BrainMessage[];
}

export interface CreateSessionRequest {
  title?: string;
  workspacePath?: string;
}

export interface CreateSessionResponse {
  sessionId: string;
  title: string;
  createdAtMs: number;
}

export interface ForkSessionRequest {
  sourceSessionId: string;
  atMessageId?: string;
  newTitle?: string;
}

export interface ForkSessionResponse {
  newSessionId: string;
  sourceSessionId: string;
  clonedMessagesCount: number;
}

export interface StartSessionInput {
  sessionId?: string;
  title?: string;
  workspacePath?: string;
}

export interface StartSessionOutput {
  sessionId: string;
  title: string;
  createdAtMs: number;
}

export interface AppendTurnInput {
  sessionId: string;
  turnId?: string;
  role: 'user' | 'assistant';
  content: string | BrainContentBlock[];
  toolResults?: BrainToolResultBlock[];
  timestampMs?: number;
}

export interface AppendTurnOutput {
  success: boolean;
  messageId: string;
  sessionId: string;
}

export interface CompleteTurnInput {
  sessionId: string;
  turnId?: string;
  assistantResponse: string;
  durationMs?: number;
  inputTokens?: number;
  outputTokens?: number;
  timestampMs?: number;
}

export interface CompleteTurnOutput {
  success: boolean;
  sessionId: string;
  totalTurns: number;
}

export interface MemorySearchInput {
  sessionId?: string;
  query: string;
  workspacePath?: string;
  limit?: number;
}

export interface MemorySearchOutput {
  memories: RetrievedMemory[];
  provenance: MemoryProvenance;
  tokenCount: number;
  serializedContext: string;
}

export interface MemoryStoreInput {
  sessionId?: string;
  label: string;
  content: string;
  scope?: string;
  relations?: Array<{ target: string; relation: string }>;
}

export interface MemoryStoreOutput {
  success: boolean;
  nodeId: string;
}

export interface ToolFeedbackIdentity {
  sessionId: string;
  turnId: string;
  toolUseId: string;
  sequence?: number;
  timestamp?: number;
}

export interface ToolFeedbackTool {
  name: string;
  status: string;
}

export interface ToolFeedbackOperation {
  path?: string;
  commandName?: string;
  affectedPaths?: string[];
}

export interface ToolFeedbackResultDetails {
  summary?: string;
  isError: boolean;
  durationMs?: number;
}

export interface ToolExecutionFeedback {
  identity: ToolFeedbackIdentity;
  tool: ToolFeedbackTool;
  operation?: ToolFeedbackOperation;
  result?: ToolFeedbackResultDetails;
  payloadHash?: string;
}

export interface ToolFeedbackResult {
  success: boolean;
  eventId: string;
  factsIngested: number;
  entitiesLinked: string[];
  isDuplicate: boolean;
}

export interface AuthoritativeSessionSummary {
  sessionId: string;
  title: string;
  messageCount: number;
  createdAtMs: number;
  updatedAtMs: number;
  workspacePath?: string;
}

export interface AuthoritativeSessionMessage {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: number;
}

export interface AuthoritativeSessionDetail {
  sessionId: string;
  title: string;
  createdAtMs: number;
  updatedAtMs: number;
  workspacePath?: string;
  messages: AuthoritativeSessionMessage[];
}

export interface ConsolidationResult {
  actionsApplied: number;
  promoted: number;
  merged: number;
  archived: number;
  pruned: number;
  durationMs: number;
  errors: string[];
}

export interface BrainModelDescriptor {
  id: string;
  name: string;
  provider: string;
  contextWindow: number;
  maxOutputTokens: number;
  supportsThinking: boolean;
  supportsTools: boolean;
  isDefault: boolean;
}

/**
 * Pure Semantic Brain Backend Interface
 * Decoupled from transport mechanics, cursor coordinates, and UI rendering buffers.
 */
export interface BrainBackend {
  startSession(input: StartSessionInput): Promise<StartSessionOutput>;
  appendTurn(input: AppendTurnInput): Promise<AppendTurnOutput>;
  completeTurn(input: CompleteTurnInput): Promise<CompleteTurnOutput>;
  searchMemory(input: MemorySearchInput): Promise<MemorySearchOutput>;
  storeMemory(input: MemoryStoreInput): Promise<MemoryStoreOutput>;
  sendToolFeedback(feedback: ToolExecutionFeedback): Promise<ToolFeedbackResult>;
  listAuthoritativeSessions(options?: { limit?: number; offset?: number; workspacePath?: string }): Promise<{ sessions: AuthoritativeSessionSummary[]; total: number }>;
  loadAuthoritativeSession(sessionId: string): Promise<AuthoritativeSessionDetail>;
  triggerConsolidation(options?: {
    promotionWeightThreshold?: number;
    pruningWeightThreshold?: number;
    stalenessAgeThresholdSecs?: number;
  }): Promise<ConsolidationResult>;
  listModels(): Promise<BrainModelDescriptor[]>;
  resolveModel(query?: string): Promise<BrainModelDescriptor>;
  isAvailable(): Promise<boolean>;
}

export interface BrainBackendClient extends BrainBackend {
  /**
   * Stream text tokens, thinking tokens, or tool calls sequentially from the Brain backend.
   */
  streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk>;

  /**
   * Memory & context retrieval RPC.
   */
  retrieveContext(req: ContextRetrieveRequest): Promise<ContextRetrieveResponse>;

  /**
   * Session CRUD and switching RPC methods.
   */
  createSession(req?: CreateSessionRequest): Promise<CreateSessionResponse>;
  loadSession(sessionId: string): Promise<{ session: BrainSession }>;
  forkSession(req: ForkSessionRequest): Promise<ForkSessionResponse>;
  archiveSession(sessionId: string): Promise<void>;
  restoreSession(sessionId: string): Promise<void>;
  renameSession(sessionId: string, newTitle: string): Promise<void>;
  pinSession(sessionId: string, pinned: boolean): Promise<void>;
  listSessions(): Promise<BrainSessionSummary[]>;

  /**
   * Best-effort wire resolution of a pending tool-permission request
   * (v1/tool/resolve). Optional: legacy fakes may omit it; the controller
   * degrades gracefully to local-only UX when absent.
   */
  resolveToolPermission?(callId: string, granted: boolean): Promise<void>;
}

/**
 * Configurable Test Double supporting functional generators.
 */
export class MockBrainBackendClient implements BrainBackendClient {
  private mockSessions: Map<string, BrainSession> = new Map();
  private sessionSeq: number = 0;

  constructor(
    private handlerOrTokens?:
      | string[]
      | ((request: BrainGenerationRequest) => AsyncIterable<BrainStreamChunk> | BrainStreamChunk[]),
    private emitError?: string
  ) {}

  /** Recorded v1/tool/resolve invocations, for controller-level assertions. */
  readonly permissionResolutions: Array<{ callId: string; granted: boolean }> = [];

  async resolveToolPermission(callId: string, granted: boolean): Promise<void> {
    this.permissionResolutions.push({ callId, granted });
  }

  async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
    if (this.emitError) {
      yield { type: 'error', error: this.emitError };
      return;
    }

    if (typeof this.handlerOrTokens === 'function') {
      for await (const chunk of this.handlerOrTokens(request)) {
        if (request.signal?.aborted) break;
        yield chunk;
      }
      return;
    }

    if (Array.isArray(this.handlerOrTokens)) {
      let inputTokens = request.messages.reduce(
        (acc, m) => acc + (typeof m.content === 'string' ? m.content.length : 10),
        0
      );
      for (const token of this.handlerOrTokens) {
        if (request.signal?.aborted) break;
        yield {
          type: 'token',
          token,
          metadata: {
            model: request.model || 'brain-engine-v1',
            inputTokens,
            outputTokens: this.handlerOrTokens.length,
          },
        };
      }
      yield { type: 'finished' };
      return;
    }

    yield {
      type: 'token',
      token: 'Default mock response',
      metadata: { model: 'mock-engine', inputTokens: 10, outputTokens: 3 },
    };
    yield { type: 'finished' };
  }

  async createSession(req?: CreateSessionRequest): Promise<CreateSessionResponse> {
    const sessionId = `01JMB8K${Date.now()}_${++this.sessionSeq}`;
    const title = req?.title || 'New Session';
    const now = Date.now();
    const session: BrainSession = {
      id: sessionId,
      title,
      archived: false,
      pinned: false,
      updatedAtMs: now,
      messages: [],
      goals: [],
    };
    this.mockSessions.set(sessionId, session);
    return { sessionId, title, createdAtMs: now };
  }

  async loadSession(sessionId: string): Promise<{ session: BrainSession }> {
    let s = this.mockSessions.get(sessionId);
    if (!s) {
      s = {
        id: sessionId,
        title: 'Mock Session',
        archived: false,
        pinned: false,
        updatedAtMs: Date.now(),
        messages: [],
        goals: [],
      };
      this.mockSessions.set(sessionId, s);
    }
    return { session: s };
  }

  async forkSession(req: ForkSessionRequest): Promise<ForkSessionResponse> {
    const src = this.mockSessions.get(req.sourceSessionId);
    const newId = `01JMB8K${Date.now()}_fork_${++this.sessionSeq}`;
    let msgs: BrainSessionMessage[] = [];
    if (src) {
      if (req.atMessageId) {
        const idx = src.messages.findIndex((m) => m.id === req.atMessageId);
        msgs = idx !== -1 ? src.messages.slice(0, idx + 1) : [...src.messages];
      } else {
        msgs = [...src.messages];
      }
    }
    const forked: BrainSession = {
      id: newId,
      title: req.newTitle || `${src?.title || 'Session'} (fork)`,
      archived: false,
      pinned: false,
      updatedAtMs: Date.now(),
      messages: msgs,
      goals: src ? [...src.goals] : [],
    };
    this.mockSessions.set(newId, forked);
    return {
      newSessionId: newId,
      sourceSessionId: req.sourceSessionId,
      clonedMessagesCount: msgs.length,
    };
  }

  async archiveSession(sessionId: string): Promise<void> {
    const s = this.mockSessions.get(sessionId);
    if (s) {
      s.archived = true;
      s.updatedAtMs = Date.now();
    }
  }

  async restoreSession(sessionId: string): Promise<void> {
    const s = this.mockSessions.get(sessionId);
    if (s) {
      s.archived = false;
      s.updatedAtMs = Date.now();
    }
  }

  async renameSession(sessionId: string, newTitle: string): Promise<void> {
    const s = this.mockSessions.get(sessionId);
    if (s) {
      s.title = newTitle;
      s.updatedAtMs = Date.now();
    }
  }

  async pinSession(sessionId: string, pinned: boolean): Promise<void> {
    const s = this.mockSessions.get(sessionId);
    if (s) {
      s.pinned = pinned;
      s.updatedAtMs = Date.now();
    }
  }

  async listSessions(): Promise<BrainSessionSummary[]> {
    return Array.from(this.mockSessions.values()).map((s) => ({
      id: s.id,
      title: s.title,
      updatedAtMs: s.updatedAtMs,
      pinned: s.pinned,
      archived: s.archived,
    }));
  }

  async isAvailable(): Promise<boolean> {
    return true;
  }

  async startSession(input: StartSessionInput): Promise<StartSessionOutput> {
    const res = await this.createSession({ title: input.title, workspacePath: input.workspacePath });
    if (input.sessionId) {
      const s = this.mockSessions.get(res.sessionId);
      if (s) {
        this.mockSessions.delete(res.sessionId);
        s.id = input.sessionId;
        this.mockSessions.set(input.sessionId, s);
        return { sessionId: input.sessionId, title: s.title, createdAtMs: res.createdAtMs };
      }
    }
    return { sessionId: res.sessionId, title: res.title, createdAtMs: res.createdAtMs };
  }

  async appendTurn(input: AppendTurnInput): Promise<AppendTurnOutput> {
    const s = this.mockSessions.get(input.sessionId);
    const msgId = `msg_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
    const textContent = typeof input.content === 'string' ? input.content : JSON.stringify(input.content);
    if (s) {
      s.messages.push({
        id: msgId,
        role: input.role,
        content: textContent,
        timestampMs: input.timestampMs || Date.now(),
      });
      s.updatedAtMs = Date.now();
    }
    return { success: true, messageId: msgId, sessionId: input.sessionId };
  }

  async completeTurn(input: CompleteTurnInput): Promise<CompleteTurnOutput> {
    const s = this.mockSessions.get(input.sessionId);
    if (s) {
      s.messages.push({
        id: `msg_comp_${Date.now()}`,
        role: 'assistant',
        content: input.assistantResponse,
        timestampMs: input.timestampMs || Date.now(),
      });
      s.updatedAtMs = Date.now();
      return { success: true, sessionId: input.sessionId, totalTurns: s.messages.length };
    }
    return { success: true, sessionId: input.sessionId, totalTurns: 1 };
  }

  async searchMemory(input: MemorySearchInput): Promise<MemorySearchOutput> {
    const res = await this.retrieveContext({
      session_id: input.sessionId || 'default',
      query: input.query,
      workspace_id: input.workspacePath,
      limit: input.limit,
    });
    return {
      memories: res.memories,
      provenance: res.provenance,
      tokenCount: res.token_count,
      serializedContext: res.serialized_context,
    };
  }

  async storeMemory(input: MemoryStoreInput): Promise<MemoryStoreOutput> {
    const nodeId = `node_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
    return { success: true, nodeId };
  }

  async retrieveContext(req: ContextRetrieveRequest): Promise<ContextRetrieveResponse> {
    return {
      memories: [],
      provenance: {
        count: 0,
        sources: [],
        channels: [],
        epoch_id: `epoch-${Date.now()}`,
      },
      token_count: 0,
      serialized_context: '',
    };
  }
}

