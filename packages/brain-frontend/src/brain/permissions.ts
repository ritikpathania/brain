import { isRecord } from './protocol/json.js';

export type PendingPermission = Readonly<{
  permissionRequestId: string;
  callId: string;
  sessionId: string;
  generationId: string;
  toolName: string;
  input: Readonly<Record<string, unknown>>;
}>;

export type PermissionTransport = Readonly<{
  requestSingleFlight(action: string, body: unknown): Promise<unknown>;
}>;

export class PermissionResolutionError extends Error {
  public constructor(
    public readonly code: 'duplicate_permission_request' | 'permission_not_found' | 'permission_already_settled' | 'malformed_permission_response',
    message: string,
  ) {
    super(`${code}: ${message}`);
    this.name = 'PermissionResolutionError';
  }
}

export class PermissionController {
  private readonly pending = new Map<string, PendingPermission>();
  private readonly settled = new Map<string, PendingPermission>();

  public constructor(private readonly transport: PermissionTransport) {}

  public register(permission: PendingPermission): void {
    if (this.pending.has(permission.permissionRequestId) || this.settled.has(permission.permissionRequestId)) {
      throw new PermissionResolutionError('duplicate_permission_request', 'permission request is already known');
    }
    this.pending.set(permission.permissionRequestId, Object.freeze({ ...permission, input: Object.freeze({ ...permission.input }) }));
  }

  public get(permissionRequestId: string): PendingPermission | undefined {
    return this.pending.get(permissionRequestId);
  }

  /** Definitive terminal/recovery boundary: no historical permission archive. */
  public abandonGeneration(sessionId: string, generationId: string): void {
    for (const store of [this.pending, this.settled]) {
      for (const [id, permission] of store) {
        if (permission.sessionId === sessionId && permission.generationId === generationId) store.delete(id);
      }
    }
  }

  public async resolve(permissionRequestId: string, decision: 'allow' | 'deny'): Promise<void> {
    if (this.settled.has(permissionRequestId)) {
      throw new PermissionResolutionError('permission_already_settled', 'permission decision was already submitted');
    }
    const permission = this.pending.get(permissionRequestId);
    if (permission === undefined) {
      throw new PermissionResolutionError('permission_not_found', 'no pending permission request matches the decision');
    }

    // Mark before I/O so a second local event cannot issue a duplicate request.
    this.pending.delete(permissionRequestId);
    this.settled.set(permissionRequestId, permission);
    const response = await this.transport.requestSingleFlight('v1/tool/resolve', {
      permission_request_id: permission.permissionRequestId,
      call_id: permission.callId,
      session_id: permission.sessionId,
      generation_id: permission.generationId,
      granted: decision === 'allow',
    });
    if (!isRecord(response) || response.type !== 'resolved' || response.status !== 'ok') {
      throw new PermissionResolutionError('malformed_permission_response', 'daemon did not acknowledge the permission settlement');
    }
  }
}
