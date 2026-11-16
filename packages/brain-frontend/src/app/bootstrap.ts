import {
  createHandshakeRequest,
  decodeHandshakeResponse,
  type NegotiatedCapabilities,
} from '../brain/handshake.js';
import { GenerationClient } from '../brain/generation.js';
import { PermissionController } from '../brain/permissions.js';
import { BrainConnectionRecovery } from '../brain/recovery.js';
import { SessionClient } from '../brain/sessions.js';
import { UdsClient } from '../brain/transport/udsClient.js';

export type BrainFrontendBootstrapOptions = Readonly<{
  socketPath: string;
  frontendVersion: string;
  frontendBuildId?: string | null;
  connectTimeoutMs?: number;
}>;

/**
 * Headless startup boundary for the first-party frontend. It proves the
 * frontend's handshake and authority relationship without deciding how a
 * future terminal presentation should render state.
 */
export class BrainFrontendBootstrap {
  public readonly transport: UdsClient;
  public readonly sessions: SessionClient;
  public readonly generations: GenerationClient;
  public readonly permissions: PermissionController;
  public readonly recovery: BrainConnectionRecovery;

  private negotiated: NegotiatedCapabilities | undefined;
  private readonly frontendVersion: string;
  private readonly frontendBuildId: string | null;

  public constructor(options: BrainFrontendBootstrapOptions) {
    this.transport = new UdsClient({
      socketPath: options.socketPath,
      ...(options.connectTimeoutMs === undefined ? {} : { connectTimeoutMs: options.connectTimeoutMs }),
    });
    this.sessions = new SessionClient(this.transport);
    this.generations = new GenerationClient(this.transport);
    this.permissions = new PermissionController(this.transport);
    this.frontendVersion = options.frontendVersion;
    this.frontendBuildId = options.frontendBuildId ?? null;
    this.recovery = new BrainConnectionRecovery({
      // Allow a short daemon restart in interactive use while keeping retries
      // bounded. Only handshake/history reads participate in this loop.
      maxAttempts: 6,
      retryDelayMs: 500,
      handshake: () => this.handshake(),
      reloadSession: async sessionId => {
        const session = await this.sessions.load(sessionId);
        return Object.freeze({ sessionId: session.sessionId });
      },
    });
  }

  public async start(): Promise<NegotiatedCapabilities> {
    await this.handshake();
    this.recovery.markReady();
    return this.capabilities();
  }

  public markDisconnected(reason: Error): void {
    this.negotiated = undefined;
    this.recovery.markDisconnected(reason);
  }

  public async recover(sessionId: string): Promise<Readonly<{ kind: 'recovered'; sessionId: string }>> {
    return this.recovery.recover(sessionId);
  }

  public capabilities(): NegotiatedCapabilities {
    if (this.negotiated === undefined) throw new Error('frontend_not_ready: handshake has not completed');
    return this.negotiated;
  }

  public async shutdown(): Promise<void> {
    // The UDS client uses short-lived sockets. There is no local durable
    // session or transcript state to flush in this headless phase.
    this.negotiated = undefined;
  }

  private async handshake(): Promise<void> {
    const response = await this.transport.request(
      'v1/frontend/handshake',
      createHandshakeRequest({ version: this.frontendVersion, buildId: this.frontendBuildId }),
    );
    // UdsClient intentionally returns only the decoded response body. Re-wrap
    // it in the owned versioned envelope shape for the strict handshake decoder.
    this.negotiated = decodeHandshakeResponse({
      type: 'Response',
      status: 'success',
      body: JSON.stringify(response),
    });
  }
}
