import { randomUUID } from 'node:crypto';

import type { BrainFrontendBootstrap } from '../app/bootstrap.js';
import { BrainContextState } from '../brain/context.js';
import { GenerationStreamGuard, type GenerationRequest, type GenerationStreamEvent } from '../brain/generation.js';
import type { ConnectionState } from '../brain/recovery.js';
import type { BrainSession, BrainSessionSummary, SessionMessage } from '../brain/sessions.js';
import { UdsProtocolError } from '../brain/transport/udsClient.js';
import type { PermissionController } from '../brain/permissions.js';
import { safeToolText } from '../brain/protocol/safeText.js';
import type { BrainToolPresentation, ContextSummary, PermissionPresentation } from './tools.js';
import {
  appendNewline,
  appendToDraft,
  deletePreviousCharacter,
  emptyComposerState,
  replaceDraft,
  moveDraftCursor,
  type CursorDirection,
  type ComposerState,
} from './composer.js';
import { composerFocus, sessionPickerFocus, type FrontendFocus } from './focus.js';

export type ConversationEntry =
  | Readonly<{ kind: 'user'; id: string; text: string }>
  | Readonly<{ kind: 'assistant'; id: string; text: string; status: 'streaming' | 'complete' | 'cancelled' | 'failed' }>
  | Readonly<{ kind: 'thinking'; id: string; text: string; status: 'streaming' | 'complete' }>
  | BrainToolPresentation
  | Readonly<{ kind: 'permission'; id: string; label: string; status?: 'required' | 'allow_submitted' | 'deny_submitted' | 'failed' }>
  | Readonly<{ kind: 'error'; id: string; message: string }>
  | Readonly<{ kind: 'status'; id: string; message: string }>;

export type FrontendInteractionSnapshot = Readonly<{
  connection: ConnectionState;
  session?: BrainSession;
  sessions: readonly BrainSessionSummary[];
  transcript: readonly ConversationEntry[];
  composer: ComposerState;
  generating: boolean;
  cancelling: boolean;
  focus: FrontendFocus;
  sessionPickerOpen: boolean;
  selectedSessionIndex: number;
  followTranscript: boolean;
  scrollOffset: number;
  permission?: PermissionPresentation;
  contextSummary?: ContextSummary;
}>;

export type BrainFrontendRuntime = Readonly<{
  start(): Promise<unknown>;
  checkConnection?(): Promise<void>;
  permissions?: PermissionController;
  sessions: Readonly<{
    create(input?: Readonly<{ title?: string; systemPrompt?: string }>): Promise<Readonly<{ sessionId: string }>>;
    list(): Promise<readonly BrainSessionSummary[]>;
    load(sessionId: string): Promise<BrainSession>;
  }>;
  generations: Readonly<{
    stream(request: GenerationRequest): AsyncGenerator<unknown, void, void>;
    cancel(correlation: Readonly<{ sessionId: string; generationId: string }>): Promise<void>;
  }>;
  connection: Readonly<{
    epoch(): number;
    markDisconnected(error: Error): void;
    recover(sessionId: string): Promise<unknown>;
  }>;
}>;

type ActiveGeneration = Readonly<{
  generationId: string;
  sessionId: string;
  epoch: number;
  pendingDraft: string;
  guard: GenerationStreamGuard;
  acceptance: Promise<boolean>;
  acknowledgeAcceptance(accepted: boolean): void;
}>;

type StateListener = (state: FrontendInteractionSnapshot) => void;

export class FrontendInteractionError extends Error {
  public readonly code = 'generation_interrupted';

  public constructor() {
    super('generation_interrupted: stream closed before an authoritative terminal event');
    this.name = 'FrontendInteractionError';
  }
}

export class BrainFrontendController {
  private state: FrontendInteractionSnapshot = initialInteractionState();
  private active: ActiveGeneration | undefined;
  private sessionOperationPending = false;
  private connectionCheckPending = false;
  private recovering: Promise<void> | undefined;
  private readonly listeners = new Set<StateListener>();

  public constructor(
    private readonly runtime: BrainFrontendRuntime,
    private readonly contexts = new BrainContextState(),
  ) {}

  public static fromBootstrap(bootstrap: BrainFrontendBootstrap): BrainFrontendController {
    return new BrainFrontendController({
      start: () => bootstrap.start(),
      checkConnection: async () => { await bootstrap.transport.request('v1/status', {}); },
      sessions: bootstrap.sessions,
      generations: bootstrap.generations,
      permissions: bootstrap.permissions,
      connection: {
        epoch: () => bootstrap.recovery.snapshot().epoch,
        markDisconnected: error => bootstrap.markDisconnected(error),
        recover: sessionId => bootstrap.recover(sessionId),
      },
    });
  }

  public snapshot(): FrontendInteractionSnapshot {
    return this.state;
  }

  public subscribe(listener: StateListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  public async start(): Promise<'ready' | 'failed'> {
    try {
      await this.runtime.start();
      this.update({ connection: 'ready' });
      return 'ready';
    } catch (error) {
      this.update({
        connection: 'failed',
        transcript: appendEntry(this.state.transcript, errorEntry(asError(error))),
      });
      return 'failed';
    }
  }

  public setDraft(draft: string): void {
    this.update({ composer: replaceDraft(this.state.composer, draft) });
  }

  public appendDraft(input: string): void {
    this.update({ composer: appendToDraft(this.state.composer, input) });
  }

  public insertNewline(): void {
    this.update({ composer: appendNewline(this.state.composer) });
  }

  public deleteDraftCharacter(): void {
    this.update({ composer: deletePreviousCharacter(this.state.composer) });
  }

  public moveComposerCursor(direction: CursorDirection): void {
    if (this.active !== undefined) return;
    this.update({ composer: moveDraftCursor(this.state.composer, direction) });
  }

  public async createSession(): Promise<void> {
    await this.performSessionOperation(async () => {
      const created = await this.runtime.sessions.create();
      if (this.state.session !== undefined) this.contexts.clearSession(this.state.session.sessionId);
      const session: BrainSession = Object.freeze({ sessionId: created.sessionId, title: 'New session', messages: Object.freeze([]) });
      this.update({
        session,
        transcript: Object.freeze([]),
        focus: composerFocus(),
        sessionPickerOpen: false,
        selectedSessionIndex: 0,
        followTranscript: true,
        scrollOffset: 0,
        contextSummary: undefined,
      });
    });
  }

  public async openSessionPicker(): Promise<void> {
    await this.performSessionOperation(async () => {
      const sessions = await this.runtime.sessions.list();
      this.update({
        sessions: Object.freeze([...sessions]),
        sessionPickerOpen: true,
        focus: sessionPickerFocus(),
        selectedSessionIndex: 0,
      });
    });
  }

  public closeSessionPicker(): void {
    this.update({ sessionPickerOpen: false, focus: composerFocus(), selectedSessionIndex: 0 });
  }

  public moveSessionSelection(delta: -1 | 1): void {
    if (!this.state.sessionPickerOpen) return;
    const optionCount = this.state.sessions.length + 1;
    const selectedSessionIndex = (this.state.selectedSessionIndex + delta + optionCount) % optionCount;
    this.update({ selectedSessionIndex });
  }

  public async selectHighlightedSession(): Promise<void> {
    if (!this.state.sessionPickerOpen) return;
    if (this.state.selectedSessionIndex === 0) {
      await this.createSession();
      return;
    }
    const selected = this.state.sessions[this.state.selectedSessionIndex - 1];
    if (selected !== undefined) await this.selectSession(selected.sessionId);
  }

  public async selectSession(sessionId: string): Promise<void> {
    await this.performSessionOperation(async () => {
      const session = await this.runtime.sessions.load(sessionId);
      if (this.state.session !== undefined) this.contexts.clearSession(this.state.session.sessionId);
      this.contexts.clearSession(sessionId);
      this.update({
        session,
        transcript: Object.freeze(projectHistory(session.messages)),
        sessionPickerOpen: false,
        focus: composerFocus(),
        followTranscript: true,
        scrollOffset: 0,
        contextSummary: undefined,
      });
    });
  }

  public scrollUp(): void {
    this.update({ followTranscript: false, scrollOffset: Math.min(Math.max(0, this.state.transcript.length - 1), this.state.scrollOffset + 5) });
  }

  public scrollToBottom(): void {
    this.update({ followTranscript: true, scrollOffset: 0 });
  }

  public closeContext(): void {
    if (this.state.focus === 'context') this.update({ focus: composerFocus() });
  }

  public toggleToolDetail(): void {
    if (this.state.focus !== 'composer') return;
    for (let index = this.state.transcript.length - 1; index >= 0; index--) {
      const entry = this.state.transcript[index];
      if (entry?.kind === 'tool' && entry.detail) {
        this.update({ transcript: updateEntry(this.state.transcript, entry.id, e => e.kind === 'tool' ? Object.freeze({ ...e, expanded: !e.expanded }) : e) });
        return;
      }
    }
  }

  public async resolvePermission(decision: 'allow' | 'deny'): Promise<'submitted' | 'ignored'> {
    const permission = this.state.permission;
    const active = this.active;
    if (permission === undefined || active === undefined || this.state.cancelling || this.runtime.permissions === undefined) return 'ignored';
    const rowId = `${active.generationId}:${permission.requestId}`;
    // Close the interactive target before I/O; repeated key events cannot dispatch twice.
    this.update({ permission: undefined, focus: composerFocus(), transcript: updateEntry(this.state.transcript, rowId, entry => entry.kind === 'permission'
      ? Object.freeze({ ...entry, status: decision === 'allow' ? 'allow_submitted' : 'deny_submitted' }) : entry) });
    try {
      const pending = this.runtime.permissions.get(permission.requestId);
      await this.runtime.permissions.resolve(permission.requestId, decision);
      if (this.active !== active || this.runtime.connection.epoch() !== active.epoch) return 'submitted';
      if (!this.state.transcript.some(entry => entry.kind === 'permission' && entry.id === rowId)) return 'submitted';
      if (decision === 'allow' && pending !== undefined) {
        this.update({ transcript: updateEntry(this.state.transcript, `${active.generationId}:${pending.callId}`, entry => entry.kind === 'tool' && entry.status === 'pending'
          ? Object.freeze({ ...entry, status: 'running', summary: 'Approved · awaiting Brain outcome' }) : entry) });
      }
      this.refreshPermissionFocus(active);
    } catch (error) {
      if (this.active !== active || this.runtime.connection.epoch() !== active.epoch) return 'submitted';
      // A correlated tool result outranks a late ACK/error, even while the
      // generation continues streaming. Never interrupt that healthy stream.
      if (!this.state.transcript.some(entry => entry.kind === 'permission' && entry.id === rowId)) return 'submitted';
      const failure = asError(error);
      if (isTransportFailure(failure)) await this.handleGenerationFailure(active, failure);
      else {
        this.update({ transcript: updateEntry(this.state.transcript, rowId, entry => entry.kind === 'permission' ? Object.freeze({ ...entry, status: 'failed' }) : entry) });
        this.appendTranscript(Object.freeze({ kind: 'error', id: `permission-error:${randomUUID()}`, message: 'Permission decision was not confirmed. It will not be retried. Cancel this turn if needed.' }));
        this.refreshPermissionFocus(active);
      }
    }
    return 'submitted';
  }

  private refreshPermissionFocus(active: ActiveGeneration): void {
    const row = this.state.transcript.find(entry => entry.kind === 'permission' && entry.status === 'required' && entry.id.startsWith(`${active.generationId}:`));
    const requestId = row?.id.slice(active.generationId.length + 1);
    const pending = requestId === undefined ? undefined : this.runtime.permissions?.get(requestId);
    if (pending !== undefined) {
      const tool = this.state.transcript.find(entry => entry.id === `${active.generationId}:${pending.callId}`);
      this.update({ focus: 'permission', permission: Object.freeze({ requestId: pending.permissionRequestId, label: safeToolText(pending.toolName, 80, 1), summary: tool?.kind === 'tool' ? tool.action ?? 'Requested Brain operation' : 'Requested Brain operation' }) });
    } else this.update({ permission: undefined, focus: composerFocus() });
  }

  public async submit(): Promise<'submitted' | 'ignored'> {
    const session = this.state.session;
    const draft = this.state.composer.draft;
    if (session === undefined || this.active !== undefined || this.sessionOperationPending || this.state.connection !== 'ready' || draft.trim().length === 0) return 'ignored';
    if (draft.trim() === '/context') {
      this.update({ composer: emptyComposerState(), focus: 'context' });
      return 'submitted';
    }

    const generationId = randomUUID();
    const epoch = this.runtime.connection.epoch();
    let acknowledgeAcceptance!: (accepted: boolean) => void;
    const acceptance = new Promise<boolean>(resolve => { acknowledgeAcceptance = resolve; });
    const active: ActiveGeneration = Object.freeze({
      generationId,
      sessionId: session.sessionId,
      epoch,
      pendingDraft: draft,
      guard: new GenerationStreamGuard({ sessionId: session.sessionId, generationId, connectionEpoch: epoch }),
      acceptance,
      acknowledgeAcceptance,
    });
    this.active = active;
    this.contexts.begin(active.sessionId, generationId);
    this.update({ composer: emptyComposerState(), generating: true, cancelling: false });

    try {
      for await (const raw of this.runtime.generations.stream({
        sessionId: session.sessionId,
        generationId,
        messages: [{ role: 'user', content: draft }],
      })) {
        const event = this.ingestGenerationFrame(generationId, raw, epoch);
        if (event.terminal !== undefined) break;
      }
      if (!active.guard.isTerminal()) throw new FrontendInteractionError();
    } catch (error) {
      await this.handleGenerationFailure(active, asError(error));
    }
    return 'submitted';
  }

  public async cancel(): Promise<'requested' | 'ignored'> {
    const active = this.active;
    if (active === undefined || this.state.cancelling) return 'ignored';
    this.update({ cancelling: true, permission: undefined, focus: composerFocus() });
    try {
      // The independent cancel socket can overtake stream setup. Brain registers
      // the generation before stream_start; an earlier cancel is an acknowledged no-op.
      // Retain one intent only for this active epoch, never retry/replay it after recovery.
      const accepted = await active.acceptance;
      if (!accepted || this.active !== active || this.runtime.connection.epoch() !== active.epoch) return 'ignored';
      await this.runtime.generations.cancel({ sessionId: active.sessionId, generationId: active.generationId });
      return 'requested';
    } catch (error) {
      if (this.active !== active || this.runtime.connection.epoch() !== active.epoch) return 'ignored';
      this.update({ cancelling: false, transcript: appendEntry(this.state.transcript, errorEntry(asError(error))) });
      this.refreshPermissionFocus(active);
      return 'ignored';
    }
  }

  public ingestGenerationFrame(generationId: string, raw: unknown, epoch: number): GenerationStreamEvent {
    const active = this.active;
    if (active === undefined || active.generationId !== generationId || active.epoch !== epoch || this.runtime.connection.epoch() !== epoch) {
      throw new Error('stale_generation: generation frame does not own the current frontend epoch');
    }
    const event = active.guard.accept(raw, epoch);
    if (event.context !== undefined) this.contexts.record(active.sessionId, active.generationId, event.context);
    this.projectGenerationEvent(active, event);
    return event;
  }

  private projectGenerationEvent(active: ActiveGeneration, event: GenerationStreamEvent): void {
    switch (event.type) {
      case 'stream_start':
        active.acknowledgeAcceptance(true);
        this.appendTranscript(Object.freeze({ kind: 'user', id: `${active.generationId}:user`, text: active.pendingDraft }));
        return;
      case 'thinking_start':
        this.appendTranscript(Object.freeze({ kind: 'thinking', id: `${active.generationId}:thinking`, text: '', status: 'streaming' }));
        return;
      case 'thinking_delta':
        this.update({ transcript: updateEntry(this.state.transcript, `${active.generationId}:thinking`, entry => entry.kind === 'thinking'
          ? Object.freeze({ ...entry, text: `${entry.text}${event.thinking ?? ''}` })
          : entry) });
        return;
      case 'thinking_end':
        this.update({ transcript: updateEntry(this.state.transcript, `${active.generationId}:thinking`, entry => entry.kind === 'thinking'
          ? Object.freeze({ ...entry, status: 'complete' })
          : entry) });
        return;
      case 'token':
      case 'text_delta':
        this.appendAssistantText(active.generationId, event.text ?? '');
        return;
      case 'tool_use':
        if (event.tool !== undefined) {
          this.appendTranscript(Object.freeze({ kind: 'tool', id: `${active.generationId}:${event.tool.callId}`, label: event.tool.name, status: 'pending', action: event.tool.summary }));
        }
        return;
      case 'tool_result':
      case 'tool_denied':
        if (event.tool !== undefined) {
          const transcript = updateEntry(this.state.transcript, `${active.generationId}:${event.tool.callId}`, entry => entry.kind === 'tool'
            ? Object.freeze({ ...entry, status: event.tool!.status, summary: event.tool!.summary, detail: event.tool!.status === 'succeeded' ? event.tool!.output : undefined })
            : entry);
          this.update({ transcript: Object.freeze(transcript.filter(entry => entry.id !== `${active.generationId}:${event.permissionRequestId}`)) });
          this.refreshPermissionFocus(active);
        }
        return;
      case 'tool_permission_requested':
      case 'permission_request':
        if (event.permission !== undefined) {
          this.runtime.permissions?.register({ permissionRequestId: event.permission.permissionRequestId, callId: event.permission.callId, sessionId: active.sessionId, generationId: active.generationId, toolName: event.permission.name, input: event.permission.input });
          this.appendTranscript(Object.freeze({ kind: 'permission', id: `${active.generationId}:${event.permission.permissionRequestId}`, label: safeToolText(event.permission.name, 80, 1), status: 'required' }));
          this.refreshPermissionFocus(active);
        }
        return;
      case 'finished':
      case 'cancelled':
      case 'error':
        this.settleGeneration(active, event.terminal ?? 'failed');
        return;
      case 'stream_end':
        return;
    }
  }

  private appendAssistantText(generationId: string, text: string): void {
    const id = `${generationId}:assistant`;
    const existing = this.state.transcript.some(entry => entry.id === id);
    if (!existing) {
      this.appendTranscript(Object.freeze({ kind: 'assistant', id, text, status: 'streaming' }));
      return;
    }
    this.update({ transcript: updateEntry(this.state.transcript, id, entry => entry.kind === 'assistant'
      ? Object.freeze({ ...entry, text: `${entry.text}${text}` })
      : entry) });
  }

  private settleGeneration(active: ActiveGeneration, terminal: 'completed' | 'cancelled' | 'failed'): void {
    active.acknowledgeAcceptance(false);
    const unresolvedTool = this.state.transcript.some(entry => entry.kind === 'tool' && (entry.status === 'pending' || entry.status === 'running') && entry.id.startsWith(`${active.generationId}:`));
    const assistantId = `${active.generationId}:assistant`;
    let transcript = updateEntry(Object.freeze(this.state.transcript.filter(entry => entry.kind !== 'permission' || !entry.id.startsWith(`${active.generationId}:`))), `${active.generationId}:thinking`, entry => entry.kind === 'thinking'
      ? Object.freeze({ ...entry, status: 'complete' })
      : entry);
    if (terminal === 'completed') {
      transcript = updateEntry(transcript, assistantId, entry => entry.kind === 'assistant'
        ? Object.freeze({ ...entry, status: 'complete' })
        : entry);
    } else {
      const status = terminal === 'cancelled' ? 'cancelled' : 'failed';
      const existing = transcript.some(entry => entry.id === assistantId);
      transcript = existing
        ? updateEntry(transcript, assistantId, entry => entry.kind === 'assistant' ? Object.freeze({ ...entry, status }) : entry)
        : appendEntry(transcript, Object.freeze({ kind: 'assistant', id: assistantId, text: '', status }));
    }
    transcript = Object.freeze(transcript.map(entry => entry.kind === 'tool' && (entry.status === 'pending' || entry.status === 'running')
      && entry.id.startsWith(`${active.generationId}:`)
      ? Object.freeze({ ...entry, status: terminal === 'cancelled' ? 'cancelled' as const : terminal === 'failed' ? 'failed' as const : 'interrupted' as const,
        summary: 'Generation settled before a tool outcome was received' }) : entry));
    this.contexts.settle(active.sessionId, active.generationId, terminal === 'completed' && unresolvedTool ? 'failed' : terminal);
    this.runtime.permissions?.abandonGeneration(active.sessionId, active.generationId);
    const context = this.contexts.latestForSession(active.sessionId);
    this.active = undefined;
    this.update({ transcript, generating: false, cancelling: false, permission: undefined, focus: composerFocus(),
      contextSummary: context === undefined ? undefined : Object.freeze({ retrievalCount: context.retrievalCount, contextTokenCount: context.contextTokenCount }) });
  }

  private async handleGenerationFailure(active: ActiveGeneration, error: Error): Promise<void> {
    active.acknowledgeAcceptance(false);
    if (this.active?.generationId !== active.generationId) return;
    const transcript = this.state.transcript
      .filter(entry => entry.kind !== 'permission' || !entry.id.startsWith(`${active.generationId}:`))
      .map(entry => {
        if (!entry.id.startsWith(`${active.generationId}:`)) return entry;
        if (entry.kind === 'thinking') return Object.freeze({ ...entry, status: 'complete' as const });
        if (entry.kind === 'tool' && (entry.status === 'pending' || entry.status === 'running')) return Object.freeze({ ...entry, status: 'interrupted' as const, summary: 'Connection interrupted · outcome unknown' });
        if (entry.kind === 'assistant') {
          return Object.freeze({ ...entry, status: 'failed' as const });
        }
        return entry;
      });
    this.contexts.settle(active.sessionId, active.generationId, 'failed');
    this.runtime.permissions?.abandonGeneration(active.sessionId, active.generationId);
    this.active = undefined;
    this.update({
      composer: replaceDraft(this.state.composer, active.pendingDraft),
      generating: false,
      cancelling: false,
      permission: undefined,
      focus: composerFocus(),
      transcript: appendEntry(transcript, errorEntry(error)),
    });
    if (isTransportFailure(error) || error instanceof FrontendInteractionError) {
      await this.recoverConnection(active.sessionId, error);
    }
  }

  /** Idle liveness checks are read-only and can never replay a user action. */
  public async refreshConnection(): Promise<void> {
    const session = this.state.session;
    if (this.runtime.checkConnection === undefined || session === undefined || this.state.connection !== 'ready'
      || this.active !== undefined || this.sessionOperationPending || this.connectionCheckPending) return;
    const epoch = this.runtime.connection.epoch();
    this.connectionCheckPending = true;
    try {
      await this.runtime.checkConnection();
    } catch (error) {
      // A read-only probe belongs to the session/epoch that started it, not a later conversation.
      if (this.runtime.connection.epoch() !== epoch || this.state.session?.sessionId !== session.sessionId
        || this.state.connection !== 'ready' || this.active !== undefined || this.sessionOperationPending) return;
      const failure = asError(error);
      if (isTransportFailure(failure)) await this.recoverConnection(session.sessionId, failure);
      else this.appendTranscript(errorEntry(failure));
    } finally {
      this.connectionCheckPending = false;
    }
  }

  private async performSessionOperation(operation: () => Promise<void>): Promise<void> {
    if (this.state.connection !== 'ready' || this.active !== undefined || this.sessionOperationPending) return;
    this.sessionOperationPending = true;
    try {
      await operation();
    } catch (error) {
      const failure = asError(error);
      this.appendTranscript(errorEntry(failure));
      const sessionId = this.state.session?.sessionId;
      if (sessionId !== undefined && isTransportFailure(failure)) await this.recoverConnection(sessionId, failure);
    } finally {
      this.sessionOperationPending = false;
    }
  }

  private async recoverConnection(sessionId: string, error: Error): Promise<void> {
    if (this.recovering !== undefined) return this.recovering;
    const recovery = this.performRecovery(sessionId, error);
    this.recovering = recovery;
    try {
      await recovery;
    } finally {
      this.recovering = undefined;
    }
  }

  private async performRecovery(sessionId: string, error: Error): Promise<void> {
    this.runtime.connection.markDisconnected(error);
    this.contexts.clearSession(sessionId);
    this.update({ connection: 'disconnected', contextSummary: undefined, permission: undefined, focus: composerFocus() });
    try {
      this.appendTranscript(Object.freeze({ kind: 'status', id: `reconnect:${Date.now()}`, message: 'Reconnecting to Brain…' }));
      this.update({ connection: 'reconnecting' });
      await this.runtime.connection.recover(sessionId);
      const session = await this.runtime.sessions.load(sessionId);
      this.update({ connection: 'ready', session, transcript: Object.freeze(projectHistory(session.messages)) });
    } catch (recoveryError) {
      this.appendTranscript(errorEntry(asError(recoveryError)));
      this.update({ connection: 'failed' });
    }
  }

  private update(change: Partial<FrontendInteractionSnapshot>): void {
    const scrollOffset = change.transcript !== undefined && change.scrollOffset === undefined
      && !this.state.followTranscript && change.followTranscript !== true
      ? retainedScrollOffset(this.state, change.transcript)
      : change.scrollOffset;
    this.state = Object.freeze({ ...this.state, ...change, ...(scrollOffset === undefined ? {} : { scrollOffset }) });
    for (const listener of this.listeners) listener(this.state);
  }

  private appendTranscript(entry: ConversationEntry): void {
    this.update({
      transcript: appendEntry(this.state.transcript, entry),
      ...(this.state.followTranscript ? {} : { scrollOffset: this.state.scrollOffset + 1 }),
    });
  }
}

function retainedScrollOffset(state: FrontendInteractionSnapshot, transcript: readonly ConversationEntry[]): number {
  const end = Math.max(0, state.transcript.length - state.scrollOffset);
  for (let index = end - 1; index >= 0; index -= 1) {
    const anchor = state.transcript[index];
    const nextIndex = transcript.findIndex(entry => entry.id === anchor?.id);
    if (nextIndex >= 0) return transcript.length - nextIndex - 1;
  }
  return Math.min(state.scrollOffset, Math.max(0, transcript.length - 1));
}

function initialInteractionState(): FrontendInteractionSnapshot {
  return Object.freeze({
    connection: 'checking',
    sessions: Object.freeze([]),
    transcript: Object.freeze([]),
    composer: emptyComposerState(),
    generating: false,
    cancelling: false,
    focus: composerFocus(),
    sessionPickerOpen: false,
    selectedSessionIndex: 0,
    followTranscript: true,
    scrollOffset: 0,
  });
}

function projectHistory(messages: readonly SessionMessage[]): ConversationEntry[] {
  const entries: ConversationEntry[] = [];
  messages.forEach((message, index) => {
    if (message.role === 'system') return;
    if (message.role === 'user') entries.push(Object.freeze({ kind: 'user', id: `history-${index}`, text: message.content }));
    if (message.role === 'assistant') entries.push(Object.freeze({ kind: 'assistant', id: `history-${index}`, text: message.content, status: 'complete' }));
    if (message.role === 'thinking') entries.push(Object.freeze({ kind: 'thinking', id: `history-${index}`, text: message.content, status: 'complete' }));
    if (message.role === 'tool') {
      const tool = historyTool(message.content, `history-${index}`);
      entries.push(tool ?? Object.freeze({ kind: 'tool', id: `history-${index}`, label: 'Stored tool outcome', status: 'interrupted', summary: 'Outcome unavailable' }));
    }
  });
  return entries;
}

function historyTool(content: string, id: string): BrainToolPresentation | undefined {
  try {
    const value: unknown = JSON.parse(content);
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined;
    const record = value as Record<string, unknown>;
    if (record.type !== 'tool_event' || record.v !== 1 || typeof record.name !== 'string') return undefined;
    const outcome = record.outcome;
    if (typeof outcome !== 'string' || !['success', 'error', 'cancelled', 'timed_out', 'denied'].includes(outcome)) return undefined;
    return Object.freeze({ kind: 'tool', id, label: safeToolText(record.name, 80, 1),
      status: outcome === 'success' ? 'succeeded' : outcome === 'cancelled' ? 'cancelled' : outcome === 'timed_out' ? 'timed_out' : 'failed',
      summary: outcome === 'denied' ? 'Permission denied' : undefined,
      detail: outcome === 'success' && typeof record.output === 'string' ? safeToolText(record.output) : undefined });
  } catch { return undefined; }
}

function appendEntry(entries: readonly ConversationEntry[], entry: ConversationEntry): readonly ConversationEntry[] {
  return Object.freeze([...entries, entry]);
}

function updateEntry(entries: readonly ConversationEntry[], id: string, update: (entry: ConversationEntry) => ConversationEntry): readonly ConversationEntry[] {
  return Object.freeze(entries.map(entry => entry.id === id ? update(entry) : entry));
}

function errorEntry(error: Error): ConversationEntry {
  return Object.freeze({ kind: 'error', id: `error:${randomUUID()}`, message: presentError(error) });
}

function isTransportFailure(error: Error): boolean {
  return (error instanceof UdsProtocolError && error.code === 'connection_unavailable') || error.message.includes('connection_unavailable');
}

function presentError(error: Error): string {
  if (error.message.includes('connection_unavailable')) return 'Daemon unavailable. Check the Brain daemon and retry.';
  if (error.message.includes('incompatible') || error.message.includes('unsupported_protocol_version') || error.message.includes('missing_required_capability')) return 'Brain daemon is incompatible with this frontend.';
  if (error.message.includes('session_not_found') || error.message.includes('session_missing_after_reconnect')) return 'The selected Brain session no longer exists.';
  if (error.message.includes('reconnect_exhausted')) return 'Could not reconnect to Brain. Restart the daemon and reopen this frontend.';
  if (error instanceof FrontendInteractionError) return 'Generation interrupted. Brain history will be reloaded; submit again only when ready.';
  if (error.name === 'GenerationStreamError' || error instanceof UdsProtocolError) return 'Brain sent an invalid response. Check the daemon and retry.';
  return 'Brain could not complete this action. Retry when the daemon is ready.';
}

function asError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}
