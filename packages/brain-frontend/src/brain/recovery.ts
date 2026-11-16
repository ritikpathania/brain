import { FrontendHandshakeError } from './handshake.js';
import { SessionNotFoundError } from './sessions.js';

export const BRAIN_NEVER_AUTOMATICALLY_RETRIED = Object.freeze([
  'generation.submit',
  'generation.cancel',
  'permission.resolve',
  'tool.execute',
] as const);

export type ConnectionState = 'checking' | 'ready' | 'disconnected' | 'reconnecting' | 'incompatible' | 'failed';

export type RecoveryDependencies = Readonly<{
  handshake(): Promise<void>;
  reloadSession(sessionId: string): Promise<Readonly<{ sessionId: string }>>;
  maxAttempts?: number;
  retryDelayMs?: number;
}>;

export type RecoverySnapshot = Readonly<{
  state: ConnectionState;
  epoch: number;
  reason?: Error;
}>;

export class RecoveryError extends Error {
  public constructor(
    public readonly code: 'incompatible_after_reconnect' | 'session_missing_after_reconnect' | 'reconnect_exhausted',
    message: string,
  ) {
    super(`${code}: ${message}`);
    this.name = 'RecoveryError';
  }
}

export class BrainConnectionRecovery {
  private state: ConnectionState = 'checking';
  private epoch = 0;
  private reason: Error | undefined;
  private readonly maxAttempts: number;
  private readonly retryDelayMs: number;

  public constructor(private readonly dependencies: RecoveryDependencies) {
    this.maxAttempts = dependencies.maxAttempts ?? 3;
    this.retryDelayMs = dependencies.retryDelayMs ?? 25;
  }

  public markReady(): void {
    this.state = 'ready';
    this.reason = undefined;
  }

  public markDisconnected(reason: Error): void {
    this.epoch += 1;
    this.state = 'disconnected';
    this.reason = reason;
  }

  public snapshot(): RecoverySnapshot {
    return Object.freeze({
      state: this.state,
      epoch: this.epoch,
      ...(this.reason === undefined ? {} : { reason: this.reason }),
    });
  }

  public async recover(sessionId: string): Promise<Readonly<{ kind: 'recovered'; sessionId: string }>> {
    if (this.state !== 'disconnected') {
      throw new Error(`recovery_not_available: current state is '${this.state}'`);
    }
    this.state = 'reconnecting';
    let lastFailure: Error | undefined;
    for (let attempt = 0; attempt < this.maxAttempts; attempt += 1) {
      try {
        await this.dependencies.handshake();
        const session = await this.dependencies.reloadSession(sessionId);
        if (session.sessionId !== sessionId) {
          throw new Error('session_recovery_mismatch: daemon reloaded a different session');
        }
        this.markReady();
        return Object.freeze({ kind: 'recovered', sessionId });
      } catch (error) {
        lastFailure = error instanceof Error ? error : new Error(String(error));
        if (isIncompatibleHandshake(lastFailure)) {
          this.state = 'incompatible';
          this.reason = lastFailure;
          throw new RecoveryError('incompatible_after_reconnect', lastFailure.message);
        }
        if (lastFailure instanceof SessionNotFoundError) {
          this.state = 'failed';
          this.reason = lastFailure;
          throw new RecoveryError('session_missing_after_reconnect', lastFailure.message);
        }
        if (attempt + 1 < this.maxAttempts) await delay(this.retryDelayMs);
      }
    }
    this.state = 'failed';
    this.reason = lastFailure;
    throw new RecoveryError('reconnect_exhausted', lastFailure?.message ?? 'reconnect failed');
  }
}

function isIncompatibleHandshake(error: Error): boolean {
  return error instanceof FrontendHandshakeError && [
    'unsupported_frontend_mode',
    'unsupported_protocol_version',
    'missing_required_capability',
  ].includes(error.code);
}

function delay(milliseconds: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, milliseconds));
}
