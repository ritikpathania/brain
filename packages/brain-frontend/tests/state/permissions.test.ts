import { describe, expect, test } from 'bun:test';

import {
  PermissionController,
  PermissionResolutionError,
} from '../../src/brain/permissions.js';

const permission = {
  permissionRequestId: 'permission-1',
  callId: 'call-1',
  sessionId: '01ARZ3NDEKTSV4RRFFQ69G5FAV',
  generationId: 'generation-1',
  toolName: 'bash',
  input: { command: 'printf ok' },
};

describe('permission controller', () => {
  test('abandoned generation permissions are unavailable and cannot dispatch', async () => {
    let calls = 0;
    const controller = new PermissionController({ requestSingleFlight: async () => { calls++; return { type: 'resolved', status: 'ok' }; } });
    controller.register(permission);
    controller.abandonGeneration(permission.sessionId, permission.generationId);
    expect(controller.get(permission.permissionRequestId)).toBeUndefined();
    await expect(controller.resolve(permission.permissionRequestId, 'deny')).rejects.toMatchObject({ code: 'permission_not_found' });
    expect(calls).toBe(0);
  });

  test('failed dispatch is not retried and denies remain exact-once', async () => {
    let calls = 0;
    const controller = new PermissionController({ requestSingleFlight: async () => { calls++; throw new Error('connection lost after dispatch'); } });
    controller.register(permission);
    await expect(controller.resolve(permission.permissionRequestId, 'deny')).rejects.toThrow('connection lost');
    await expect(controller.resolve(permission.permissionRequestId, 'allow')).rejects.toMatchObject({ code: 'permission_already_settled' });
    expect(calls).toBe(1);
  });
  test('submits one canonical Brain resolution and rejects a duplicate local decision', async () => {
    const requests: Array<{ action: string; body: unknown }> = [];
    const controller = new PermissionController({
      requestSingleFlight: async (action, body) => {
        requests.push({ action, body });
        return { type: 'resolved', status: 'ok' };
      },
    });

    controller.register(permission);
    await controller.resolve(permission.permissionRequestId, 'allow');

    expect(requests).toEqual([{
      action: 'v1/tool/resolve',
      body: {
        permission_request_id: permission.permissionRequestId,
        call_id: permission.callId,
        session_id: permission.sessionId,
        generation_id: permission.generationId,
        granted: true,
      },
    }]);
    await expect(controller.resolve(permission.permissionRequestId, 'allow'))
      .rejects.toMatchObject({ code: 'permission_already_settled' } satisfies Partial<PermissionResolutionError>);
  });
});
