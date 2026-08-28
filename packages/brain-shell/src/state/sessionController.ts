/**
 * Turn-loop owner for the shell. Non-React class exposing an immutable
 * ShellSnapshot through subscribe/getSnapshot (useSyncExternalStore seam).
 * UI actions enter here; socket access stays behind the client seam.
 */
import type {
  BrainBackendClient,
  BrainGenerationRequest,
  BrainModelDescriptor,
  BrainSessionSummary,
  BrainStreamChunk,
  ContextTelemetry,
  MemorySearchResult,
  TranscriptSearchMatch,
} from '../client/BrainBackendClient.js';
import { sessionToRows } from './sessionReplay.js';
import { normalizeMessagesForBrain } from '../adapter/brainCallModel.js';
import { createUserMessage } from '../contracts/messages.js';
import type { TranscriptRow } from '../contracts/messages.js';
import type { LiveStreamView } from '../contracts/streaming.js';
import {
  TwoStageTypewriterQueue,
  TYPEWRITER_TICK_MS,
  TYPEWRITER_CHARS_PER_TICK,
} from '../adapter/streaming/TwoStageTypewriterQueue.js';
import { chunkToTurnEvent } from '../adapter/chunkToTurnEvents.js';
import {
  ConnectionMonitor,
  type ConnectionState,
} from './connectionMonitor.js';
import { probeDaemonSocket } from '../client/probeDaemonSocket.js';
import {
  addAllowRule,
  matchingRuleIndex,
  primaryInputString,
  readAllowRules,
} from './permissionRules.js';
import { BrainTurnTransformer } from '../adapter/BrainTurnTransformer.js';
import type { BrainTurnEvent } from '../adapter/BrainTurnEvents.js';
import { turnToRows } from '../ui/transcript/toRows.js';
import { resolveDirectResumeTarget } from '../ui/overlays/resumePickerLogic.js';

export interface PendingPermissionView {
  callId: string;
  toolName: string;
  input: Record<string, unknown>;
  reason?: string;
}

export interface ShellSnapshot {
  rows: TranscriptRow[];
  live: LiveStreamView;
  busy: boolean;
  connectionError?: string;
  connection: ConnectionState;
  permission?: PendingPermissionView;
  activeModel?: BrainModelDescriptor;
  activeModelName: string;
}

const IDLE_LIVE: LiveStreamView = { phase: 'idle', thinkingText: '', responseText: '' };
const CONNECTION_RE = /Could not connect|socket error|disconnected/i;
const CONNECTION_LOSS_RE =
  /Could not connect|socket not found|socket error|disconnected|connection closed|RPC timeout/i;
const ABORT_RE = /abort/i;

/** Inc 15: a classified connection loss arms the monitor; aborts never do. */
function isConnectionLoss(text: string): boolean {
  return !ABORT_RE.test(text) && CONNECTION_LOSS_RE.test(text);
}

export const CONNECTION_LOSS_ROW = 'Connection lost — reconnecting…';
export const QUEUED_ROW = 'queued — will send on reconnect';
/** Frozen system row appended when a turn ends because the user escaped. */
export const INTERRUPTED_ROW = '⎿ Turn interrupted';

export class SessionController {
  private listeners = new Set<() => void>();
  private rows: TranscriptRow[] = [];
  private live: LiveStreamView = IDLE_LIVE;
  private busy = false;
  private connectionError: string | undefined;
  private sessionId: string | undefined;
  private latestTelemetry: ContextTelemetry | undefined;
  private aborter: AbortController | null = null;
  private queue = new TwoStageTypewriterQueue();
  private ticker: ReturnType<typeof setInterval> | null = null;
  private events: BrainTurnEvent[] = [];
  private sawError = false;
  private userInterrupted = false;
  private sawCancelledFinish = false;
  private thinkingStartedAt: number | null = null;
  private turnSeq = 0;
  private connection: ConnectionState = { status: 'connected' };
  private queuedInputs: string[] = [];
  private monitor: ConnectionMonitor | null = null;
  private lostDuringTurn = false;
  private activeModelDescriptor?: BrainModelDescriptor;
  private snapshot: ShellSnapshot = {
    rows: [],
    live: IDLE_LIVE,
    busy: false,
    connection: { status: 'connected' },
    activeModelName: 'brain-default',
  };

  constructor(
    private client: BrainBackendClient,
    private probeOverride?: () => Promise<boolean>,
    private delayOverride?: (ms: number) => Promise<void>,
  ) {}

  subscribe = (fn: () => void): (() => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };

  getSnapshot = (): ShellSnapshot => this.snapshot;

  /** Inc 38: Current active model descriptor if explicitly resolved/selected. */
  get activeModel(): BrainModelDescriptor | undefined {
    return this.activeModelDescriptor;
  }

  /** Inc 38: Formatted active model name for presentation. */
  get activeModelName(): string {
    return this.activeModelDescriptor?.name ?? 'brain-default';
  }

  /** B5: adopted session id, for the resume picker's current-session marker. */
  get activeSessionId(): string | undefined {
    return this.sessionId;
  }

  /** Inc 36: latest turn context retrieval provenance and telemetry. */
  get latestContextTelemetry(): ContextTelemetry | undefined {
    return this.latestTelemetry;
  }

  /** Inc 38: List available models from the authoritative backend gateway. */
  async listAvailableModels(): Promise<BrainModelDescriptor[]> {
    try {
      return await this.client.listModels();
    } catch (e) {
      this.notice(`Could not list models: ${e instanceof Error ? e.message : String(e)}`);
      return [];
    }
  }

  /** Inc 38: Resolve and switch active LLM generation model. */
  async switchModel(query?: string): Promise<BrainModelDescriptor | undefined> {
    const trimmed = query?.trim();
    try {
      const resolved = await this.client.resolveModel(
        trimmed && trimmed.length > 0 ? trimmed : undefined,
      );
      this.activeModelDescriptor = resolved;
      this.emit();
      this.notice(`Switched active model to “${resolved.name}” (${resolved.id})`);
      return resolved;
    } catch (e) {
      this.notice(`Could not resolve model: ${e instanceof Error ? e.message : String(e)}`);
      return undefined;
    }
  }

  abort(): void {
    this.aborter?.abort();
  }

  /** User-facing interrupt: aborts the wire stream and remembers why, so
   * settlement renders an interruption instead of a silent completion.
   * No-op while idle. */
  interruptTurn(): void {
    if (!this.busy || !this.aborter) return;
    this.userInterrupted = true;
    this.aborter.abort();
  }

  /** Resolve the parked dialog manually. Local UX settles first; the wire
   * verdict rides v1/tool/resolve best-effort — legacy fakes and offline
   * daemons simply omit or reject the call, while live daemons gate
   * execution on it. */
  resolvePermission(callId: string, granted: boolean): void {
    if (this.pendingPermission?.callId !== callId) return;
    const toolName = this.pendingPermission.toolName;
    this.pendingPermission = undefined;
    if (!granted) {
      this.rows = this.rows.map((r) =>
        r.kind === 'tool' && r.tool.callId === callId
          ? { ...r, tool: { ...r.tool, status: 'denied' as const } }
          : r,
      );
    }
    // Best-effort wire round-trip; local UX above is already settled.
    void this.client.resolveToolPermission?.(callId, granted)?.catch(() => {});
    this.notice(`${granted ? 'Allowed' : 'Denied'} ${toolName}`);
  }

  /** Inc 17: grant AND persist an always-allow rule derived from the pending
   * view. A save failure only skips persistence — this call is still granted. */
  resolvePermissionAlways(callId: string): void {
    const view = this.pendingPermission;
    if (view?.callId !== callId) return;
    try {
      addAllowRule({
        tool: view.toolName,
        inputPrefix: primaryInputString(view.input),
      });
    } catch {
      this.notice('Could not save the always-allow rule.');
    }
    this.resolvePermission(callId, true);
  }

  /** Saved-rule hit: never park the dialog. If the wire verdict cannot be
   * delivered, re-park so the user decides manually — a silent drop would
   * leave the daemon's waiter parked until its deny-by-default timeout. */
  private autoAllow(view: PendingPermissionView, ruleNumber: number): void {
    this.notice(`Allowed ${view.toolName} (rule ${ruleNumber})`);
    const park = (): void => {
      this.pendingPermission = view;
      this.emit();
    };
    try {
      const p = this.client.resolveToolPermission?.(view.callId, true);
      if (p) p.catch(park);
    } catch {
      park();
    }
  }

  /** Inc 15: stop the reconnect loop (AppShell cleanup effect). */
  dispose(): void {
    this.monitor?.stop();
  }

  private ensureMonitor(): ConnectionMonitor {
    if (this.monitor === null) {
      const socketPath = (this.client as { socketPath?: string }).socketPath;
      const probe =
        this.probeOverride ??
        (typeof socketPath === 'string'
          ? () => probeDaemonSocket(socketPath)
          : async () => true); // fakes without a transport restore immediately
      const delay =
        this.delayOverride ?? ((ms: number) => new Promise<void>((r) => setTimeout(r, ms)));
      this.monitor = new ConnectionMonitor({
        probe,
        delay,
        onChange: (s) => {
          this.connection = s;
          this.emit();
        },
        onRestored: () => {
          void this.drainQueue();
        },
      });
    }
    return this.monitor;
  }

  /** Classified loss anywhere: show the banner, arm the loop once. */
  private handleConnectionLoss(): void {
    if (this.connection.status === 'reconnecting') return;
    if (this.busy) this.lostDuringTurn = true;
    this.ensureMonitor().start();
  }

  private async drainQueue(): Promise<void> {
    while (this.queuedInputs.length > 0 && this.connection.status === 'connected') {
      await this.submit(this.queuedInputs[0]);
      // A second outage during this submit leaves the item queued.
      if (this.connection.status === 'connected') this.queuedInputs.shift();
    }
  }

  /** Wipe the frozen transcript (slash command /clear). */
  clear(): void {
    this.rows = [];
    this.latestTelemetry = undefined;
    this.emit();
  }

  private sysSeq = 0;

  /** Locally-generated shell output (/help, warnings). Never hits the wire. */
  notice(text: string): void {
    this.rows = [...this.rows, { kind: 'system', id: `sys:${++this.sysSeq}`, text }];
    this.emit();
  }

  /** Prior sessions for the /resume picker. */
  async listSessions(): Promise<BrainSessionSummary[]> {
    return this.client.listSessions();
  }

  /** /memory data source. Liveness-discriminated so views can render
   * offline copy vs empty copy; every transport failure collapses to ok:false. */
  async searchMemories(query: string, limit = 20): Promise<MemorySearchResult> {
    try {
      const res = await this.client.searchMemory({ query, limit });
      return { ok: true, memories: res.memories };
    } catch {
      return { ok: false };
    }
  }

  /** Inc 32: Search historical message and session transcripts across all sessions. */
  async searchTranscripts(query: string, limit = 20): Promise<TranscriptSearchMatch[]> {
    const trimmed = query.trim();
    if (!trimmed) return [];
    try {
      const res = await this.client.searchTranscripts({ query: trimmed, limit });
      return res.matches;
    } catch {
      return [];
    }
  }

  /** Inc 31: Delete a memory node by ID, removing it from storage and future context. */
  async deleteMemoryNode(nodeId: string): Promise<boolean> {
    const trimmed = nodeId.trim();
    if (!trimmed) {
      this.notice('Usage: /memory delete <node_id>');
      return false;
    }
    try {
      await this.client.deleteMemory(trimmed);
      this.notice(`Deleted memory node ${trimmed}`);
      return true;
    } catch (e) {
      this.notice(`Could not delete memory node: ${e instanceof Error ? e.message : String(e)}`);
      return false;
    }
  }

  /** Inc 35: Delete a session by ID or prefix, resetting state if active and removing from storage. */
  async deleteSession(target: string): Promise<boolean> {
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return false;
    }
    const trimmed = target.trim();
    if (!trimmed) {
      this.notice('Usage: /session delete <id_or_prefix>');
      return false;
    }

    try {
      const summaries = await this.listSessions();
      const resolution = resolveDirectResumeTarget(summaries, trimmed);

      let targetId: string;
      let targetTitle: string | undefined;

      if (resolution.type === 'exact' || resolution.type === 'unique') {
        targetId = resolution.sessionId;
        const match = summaries.find((s) => s.id === targetId);
        targetTitle = match?.title;
      } else if (resolution.type === 'ambiguous') {
        const candidateIds = resolution.candidates.map((c) => c.id.slice(0, 8)).join(', ');
        this.notice(`Ambiguous session prefix “${trimmed}” matches: ${candidateIds}`);
        return false;
      } else {
        this.notice(`No session found matching “${trimmed}”.`);
        return false;
      }

      await this.client.deleteSession(targetId);

      const titleDisplay = targetTitle ? `“${targetTitle}” ` : '';
      if (this.sessionId === targetId) {
        this.sessionId = undefined;
        this.latestTelemetry = undefined;
        this.rows = [
          {
            kind: 'system',
            id: `sys:${++this.sysSeq}`,
            text: `Deleted active session ${titleDisplay}(${targetId.slice(0, 8)})`,
          },
        ];
        this.emit();
      } else {
        this.notice(`Deleted session ${titleDisplay}(${targetId.slice(0, 8)})`);
      }
      return true;
    } catch (e) {
      this.notice(`Could not delete session: ${e instanceof Error ? e.message : String(e)}`);
      return false;
    }
  }

  /** Inc 37: Pin or unpin a session in storage and emit a confirmation notice. */
  async pinSession(target?: string, pinned = true): Promise<boolean> {
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return false;
    }
    const trimmed = (target ?? '').trim();

    try {
      const summaries = await this.listSessions();
      let targetId: string;
      let targetTitle: string | undefined;

      if (!trimmed) {
        if (!this.sessionId) {
          this.notice(`No active session to ${pinned ? 'pin' : 'unpin'}. Specify a session ID or prefix.`);
          return false;
        }
        targetId = this.sessionId;
        const match = summaries.find((s) => s.id === targetId);
        targetTitle = match?.title;
      } else {
        const resolution = resolveDirectResumeTarget(summaries, trimmed);
        if (resolution.type === 'exact' || resolution.type === 'unique') {
          targetId = resolution.sessionId;
          const match = summaries.find((s) => s.id === targetId);
          targetTitle = match?.title;
        } else if (resolution.type === 'ambiguous') {
          const candidateIds = resolution.candidates.map((c) => c.id.slice(0, 8)).join(', ');
          this.notice(`Ambiguous session prefix “${trimmed}” matches: ${candidateIds}`);
          return false;
        } else {
          this.notice(`No session found matching “${trimmed}”.`);
          return false;
        }
      }

      await this.client.pinSession(targetId, pinned);

      const titleDisplay = targetTitle ? `“${targetTitle}” ` : '';
      const actionWord = pinned ? 'Pinned' : 'Unpinned';
      this.notice(`${actionWord} session ${titleDisplay}(${targetId.slice(0, 8)})`);
      return true;
    } catch (e) {
      this.notice(`Could not ${pinned ? 'pin' : 'unpin'} session: ${e instanceof Error ? e.message : String(e)}`);
      return false;
    }
  }

  /** Inc 37: Archive a session, resetting active state if it was the active session. */
  async archiveSession(target?: string): Promise<boolean> {
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return false;
    }
    const trimmed = (target ?? '').trim();

    try {
      const summaries = await this.listSessions();
      let targetId: string;
      let targetTitle: string | undefined;

      if (!trimmed) {
        if (!this.sessionId) {
          this.notice('No active session to archive. Specify a session ID or prefix.');
          return false;
        }
        targetId = this.sessionId;
        const match = summaries.find((s) => s.id === targetId);
        targetTitle = match?.title;
      } else {
        const resolution = resolveDirectResumeTarget(summaries, trimmed);
        if (resolution.type === 'exact' || resolution.type === 'unique') {
          targetId = resolution.sessionId;
          const match = summaries.find((s) => s.id === targetId);
          targetTitle = match?.title;
        } else if (resolution.type === 'ambiguous') {
          const candidateIds = resolution.candidates.map((c) => c.id.slice(0, 8)).join(', ');
          this.notice(`Ambiguous session prefix “${trimmed}” matches: ${candidateIds}`);
          return false;
        } else {
          this.notice(`No session found matching “${trimmed}”.`);
          return false;
        }
      }

      await this.client.archiveSession(targetId);

      const titleDisplay = targetTitle ? `“${targetTitle}” ` : '';
      if (this.sessionId === targetId) {
        this.sessionId = undefined;
        this.latestTelemetry = undefined;
        this.rows = [
          {
            kind: 'system',
            id: `sys:${++this.sysSeq}`,
            text: `Archived active session ${titleDisplay}(${targetId.slice(0, 8)})`,
          },
        ];
        this.emit();
      } else {
        this.notice(`Archived session ${titleDisplay}(${targetId.slice(0, 8)})`);
      }
      return true;
    } catch (e) {
      this.notice(`Could not archive session: ${e instanceof Error ? e.message : String(e)}`);
      return false;
    }
  }

  /** Inc 37: Restore an archived session back to active visibility. */
  async restoreSession(target: string): Promise<boolean> {
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return false;
    }
    const trimmed = target.trim();
    if (!trimmed) {
      this.notice('Usage: /session restore <id_or_prefix>');
      return false;
    }

    try {
      const summaries = await this.listSessions();
      // Hard Invariant: restore must resolve against complete catalog (includeArchived: true)
      const resolution = resolveDirectResumeTarget(summaries, trimmed, { includeArchived: true });

      let targetId: string;
      let targetTitle: string | undefined;

      if (resolution.type === 'exact' || resolution.type === 'unique') {
        targetId = resolution.sessionId;
        const match = summaries.find((s) => s.id === targetId);
        targetTitle = match?.title;
      } else if (resolution.type === 'ambiguous') {
        const candidateIds = resolution.candidates.map((c) => c.id.slice(0, 8)).join(', ');
        this.notice(`Ambiguous session prefix “${trimmed}” matches: ${candidateIds}`);
        return false;
      } else {
        this.notice(`No session found matching “${trimmed}”.`);
        return false;
      }

      await this.client.restoreSession(targetId);

      const titleDisplay = targetTitle ? `“${targetTitle}” ` : '';
      this.notice(`Restored session ${titleDisplay}(${targetId.slice(0, 8)})`);
      return true;
    } catch (e) {
      this.notice(`Could not restore session: ${e instanceof Error ? e.message : String(e)}`);
      return false;
    }
  }

  /** Adopt a stored session and replay its messages as frozen rows. */
  async resumeSession(sessionId: string): Promise<void> {
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return;
    }
    try {
      const { session } = await this.client.loadSession(sessionId);
      this.sessionId = session.id;
      this.latestTelemetry = undefined;
      const replayed = sessionToRows(session);
      this.rows = [
        ...replayed,
        { kind: 'system', id: `sys:${++this.sysSeq}`, text: `Resumed “${session.title}”` },
      ];
      this.emit();
    } catch (e) {
      this.notice(`Could not resume session: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  /** Inc 29: Rename the active session in storage and record a confirmation. */
  async renameCurrentSession(newTitle: string): Promise<void> {
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return;
    }
    const trimmed = newTitle.trim();
    if (!trimmed) {
      this.notice('Usage: /rename <title>');
      return;
    }
    try {
      if (!this.sessionId) {
        const created = await this.client.createSession({ title: trimmed });
        this.sessionId = created.sessionId;
      } else {
        await this.client.renameSession(this.sessionId, trimmed);
      }
      this.notice(`Session renamed to “${trimmed}”`);
    } catch (e) {
      this.notice(`Could not rename session: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  /** Inc 29: Fork the active session, switch controller to the new session,
   * replay the cloned history, and record a confirmation. */
  async forkCurrentSession(newTitle?: string): Promise<string | undefined> {
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return undefined;
    }
    try {
      if (!this.sessionId) {
        const created = await this.client.createSession({ title: newTitle });
        this.sessionId = created.sessionId;
        this.notice(`Started session “${created.title}”`);
        return created.sessionId;
      }
      const res = await this.client.forkSession({
        sourceSessionId: this.sessionId,
        newTitle: newTitle?.trim() || undefined,
      });
      const { session } = await this.client.loadSession(res.newSessionId);
      this.sessionId = session.id;
      const replayed = sessionToRows(session);
      this.rows = [
        ...replayed,
        {
          kind: 'system',
          id: `sys:${++this.sysSeq}`,
          text: `Forked to “${session.title}” (${res.clonedMessagesCount} turns cloned)`,
        },
      ];
      this.emit();
      return session.id;
    } catch (e) {
      this.notice(`Could not fork session: ${e instanceof Error ? e.message : String(e)}`);
      return undefined;
    }
  }

  /** Inc 11: `!` bash passthrough — a standalone turn that never reaches a
   * provider. Rendered through the same reducer/projection path as live
   * agentic cards, so local and replayed rows agree by construction. */
  async runShellCommand(command: string): Promise<void> {
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return;
    }
    const trimmed = command.trim();
    if (trimmed.length === 0) return;
    this.busy = true;
    this.connectionError = undefined;
    const turnId = `turn_${++this.turnSeq}`;
    this.rows = [...this.rows, { kind: 'user', id: `user:${turnId}`, text: `! ${trimmed}` }];
    this.aborter = new AbortController();
    this.emit();
    try {
      if (this.sessionId === undefined) {
        this.sessionId = (await this.client.createSession()).sessionId;
      }
      const result = await this.client.execShell?.(this.sessionId, trimmed, this.aborter.signal);
      if (!result) {
        this.notice('This backend cannot execute shell commands.');
        return;
      }
      const callId = result.callId || `shell_${turnId}`;
      const vm = BrainTurnTransformer.transform([
        { type: 'tool_call_requested', callId, toolName: 'bash', input: result.input },
        {
          type: 'tool_result',
          callId,
          output: result.output,
          isError: result.isError,
          exitCode: result.exitCode,
          durationMs: result.durationMs,
        },
      ]);
      const projected = turnToRows(vm).filter(
        (r) => !(r.kind === 'assistant' && r.markdown.trim().length === 0),
      );
      this.rows = [...this.rows, ...projected];
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      this.notice(/abort/i.test(msg) ? 'Shell command cancelled.' : `Could not run command: ${msg}`);
    } finally {
      this.busy = false;
      this.aborter = null;
      this.emit();
    }
  }

  async submit(text: string): Promise<void> {
    // Inc 14: a submit during a live turn gets the same feedback as every
    // other busy-path entry point instead of vanishing.
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return;
    }
    // Inc 15: offline submits join the replay queue instead of failing.
    if (this.connection.status !== 'connected') {
      this.queuedInputs.push(text);
      this.notice(QUEUED_ROW);
      return;
    }
    this.busy = true;
    this.connectionError = undefined;
    this.lostDuringTurn = false;
    const turnId = `turn_${++this.turnSeq}`;
    this.rows = [...this.rows, { kind: 'user', id: `user:${turnId}`, text }];
    this.events = [{ type: 'turn_start', turnId, role: 'assistant' }];
    this.sawError = false;
    this.userInterrupted = false;
    this.sawCancelledFinish = false;
    this.queue = new TwoStageTypewriterQueue();
    this.live = { phase: 'responding', thinkingText: '', responseText: '' };
    this.aborter = new AbortController();
    this.emit();
    this.startTicker();
    try {
      if (this.sessionId === undefined) {
        this.sessionId = (await this.client.createSession()).sessionId;
      }
      const request: BrainGenerationRequest = {
        sessionId: this.sessionId,
        messages: normalizeMessagesForBrain([createUserMessage(text)]),
        model: this.activeModelDescriptor?.id,
        signal: this.aborter.signal,
      };
      for await (const chunk of this.client.streamText(request)) {
        this.handleChunk(chunk);
      }
      // An error chunk doesn't throw — the stream just ends — but the turn
      // still failed and must settle its tools accordingly. A user-initiated
      // interrupt settles as interrupted, never as a silent completion.
      const interrupted = this.userInterrupted;
      this.finishTurn(
        interrupted ? 'interrupted' : this.sawError ? 'error' : 'completed',
        interrupted || !this.lostDuringTurn ? undefined : CONNECTION_LOSS_ROW,
      );
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (isConnectionLoss(msg)) this.handleConnectionLoss();
      const racedInterrupt = this.userInterrupted && !isConnectionLoss(msg);
      this.finishTurn(
        racedInterrupt ? 'interrupted' : 'error',
        racedInterrupt
          ? undefined
          : isConnectionLoss(msg)
            ? CONNECTION_LOSS_ROW
            : msg,
      );
    }
  }

  private pendingPermission: PendingPermissionView | undefined;

  private handleChunk(chunk: BrainStreamChunk): void {
    if (chunk.type === 'finished' && chunk.status === 'cancelled') {
      this.sawCancelledFinish = true;
    }
    if (chunk.type === 'error' && chunk.error) {
      if (CONNECTION_RE.test(chunk.error)) {
        this.connectionError = chunk.error;
      }
      if (isConnectionLoss(chunk.error)) {
        this.handleConnectionLoss();
      }
    }
    // Permission requests either auto-allow from a saved rule (Inc 17) or
    // park a dialog on the snapshot; both paths end in a wire verdict via
    // v1/tool/resolve, which the daemon parks the stream awaiting.
    if (chunk.type === 'permission_request' && typeof chunk.callId === 'string') {
      const view: PendingPermissionView = {
        callId: chunk.callId,
        toolName: chunk.toolName ?? 'tool',
        input: chunk.input ?? {},
        reason: chunk.reason,
      };
      const ruleNumber =
        matchingRuleIndex(view.toolName, view.input, readAllowRules()) + 1;
      if (ruleNumber > 0) this.autoAllow(view, ruleNumber);
      else {
        this.pendingPermission = view;
        this.emit();
      }
      return;
    }
    const event = chunkToTurnEvent(chunk);
    if (event === null) return;
    // Inc 13: thinking lifecycle. The wire's daemon-measured duration wins;
    // a local bracket covers daemons that omit it.
    if (event.type === 'thinking_start') {
      this.thinkingStartedAt = Date.now();
      this.live = { ...this.live, phase: 'thinking' };
    } else if (event.type === 'thinking_end') {
      if (event.durationMs === undefined && this.thinkingStartedAt !== null) {
        event.durationMs = Math.max(0, Date.now() - this.thinkingStartedAt);
      }
      this.thinkingStartedAt = null;
    }
    this.events.push(event);
    if (event.type === 'text_delta') {
      this.queue.push(event.delta);
      // First response text ends the thinking phase visually.
      if (this.live.phase === 'thinking' || this.live.phase === 'responding') {
        this.live = { ...this.live, phase: 'responding' };
      }
    } else if (event.type === 'thinking_delta') {
      this.live = {
        ...this.live,
        phase: 'thinking',
        thinkingText: this.live.thinkingText + event.delta,
      };
    } else if (event.type === 'tool_call_requested') {
      this.live = { ...this.live, phase: 'tool', activeToolName: event.toolName };
    } else if (event.type === 'turn_error') {
      this.sawError = true;
      this.live = { ...this.live, phase: 'error', errorText: event.error };
    }
    this.emit();
  }

  private startTicker(): void {
    this.ticker = setInterval(() => {
      if (this.queue.pending === 0) return;
      const out = this.queue.drain(TYPEWRITER_CHARS_PER_TICK);
      this.live = { ...this.live, responseText: this.live.responseText + out };
      this.emit();
    }, TYPEWRITER_TICK_MS);
  }

  private stopTicker(): void {
    if (this.ticker !== null) {
      clearInterval(this.ticker);
      this.ticker = null;
    }
  }

  private finishTurn(
    status: 'completed' | 'error' | 'interrupted',
    errorText?: string,
  ): void {
    this.stopTicker();
    // Frozen rows are built solely from `events`, which already holds every
    // delta verbatim (handleChunk records them before queueing for pacing).
    // The typewriter queue feeds only the live view, which freeze discards —
    // flushing it here would re-push its pending tail (Inc 16 dedup fix).
    // The wire protocol has no tool-result frame yet, so requested calls
    // would otherwise stay 'pending' forever on the frozen card. Settle them:
    // completed turns finish their tools, errored turns cancel them.
    const settled = new Set<string>();
    for (const e of this.events) {
      if (e.type === 'tool_result' || e.type === 'tool_cancelled') settled.add(e.callId);
    }
    for (const e of [...this.events]) {
      if (e.type === 'tool_call_requested' && !settled.has(e.callId)) {
        settled.add(e.callId);
        this.events.push(
          status === 'completed'
            ? { type: 'tool_result', callId: e.callId, output: '' }
            : {
                type: 'tool_cancelled',
                callId: e.callId,
                reason: status === 'interrupted' ? 'turn interrupted' : 'turn ended',
              },
        );
      }
    }
    if (status === 'error' && errorText !== undefined) {
      this.events.push({ type: 'turn_error', error: errorText });
    }
    this.events.push({ type: 'thinking_end' });
    this.events.push({ type: 'turn_complete' });
    try {
      const vm = BrainTurnTransformer.transform(this.events);
      if (vm.telemetry) {
        this.latestTelemetry = vm.telemetry;
      }
      const projected = turnToRows(vm).filter(
        (r) => !(r.kind === 'assistant' && r.markdown.trim().length === 0),
      );
      this.rows = [...this.rows, ...projected];
    } catch {
      // Transformer mismatch must never kill the shell; keep prior rows.
    }
    if (status === 'interrupted') {
      this.rows = [
        ...this.rows,
        { kind: 'system', id: `sys:${++this.sysSeq}`, text: INTERRUPTED_ROW },
      ];
    }
    this.live = IDLE_LIVE;
    this.busy = false;
    this.aborter = null;
    this.emit();
  }

  private emit(): void {
    this.snapshot = {
      rows: this.rows,
      live: this.live,
      busy: this.busy,
      connectionError: this.connectionError,
      connection: this.connection,
      permission: this.pendingPermission,
      activeModel: this.activeModelDescriptor,
      activeModelName: this.activeModelName,
    };
    for (const fn of this.listeners) fn();
  }
}
