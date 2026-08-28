import { describe, it, expect, beforeEach } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type { BrainBackendClient, BrainSessionSummary } from '../../client/BrainBackendClient.js';

describe('SessionController lifecycle mutations (Inc 37: Pin, Unpin, Archive, Restore)', () => {
  let pinnedCalls: Array<{ id: string; pinned: boolean }>;
  let archivedCalls: string[];
  let restoredCalls: string[];
  let mockSessions: BrainSessionSummary[];
  let mockClient: Partial<BrainBackendClient>;
  let controller: SessionController;

  beforeEach(() => {
    pinnedCalls = [];
    archivedCalls = [];
    restoredCalls = [];
    mockSessions = [
      {
        id: 'session-alpha-12345678',
        title: 'Alpha Session',
        updatedAtMs: 1000,
        pinned: false,
        archived: false,
      },
      {
        id: 'session-beta-87654321',
        title: 'Beta Session',
        updatedAtMs: 2000,
        pinned: true,
        archived: false,
      },
      {
        id: 'session-archived-99999999',
        title: 'Old Archived Session',
        updatedAtMs: 500,
        pinned: false,
        archived: true,
      },
      {
        id: 'session-alpha-duplicate',
        title: 'Alpha Two',
        updatedAtMs: 1500,
        pinned: false,
        archived: false,
      },
    ];

    mockClient = {
      listSessions: async () => mockSessions,
      loadSession: async (id: string) => ({
        session: {
          id,
          title: mockSessions.find((s) => s.id === id)?.title || 'Test',
          createdAtMs: 1000,
          updatedAtMs: 1000,
          pinned: false,
          archived: false,
          messages: [],
        },
      }),
      pinSession: async (id: string, pinned: boolean) => {
        pinnedCalls.push({ id, pinned });
      },
      archiveSession: async (id: string) => {
        archivedCalls.push(id);
      },
      restoreSession: async (id: string) => {
        restoredCalls.push(id);
      },
    };

    controller = new SessionController(mockClient as BrainBackendClient);
  });

  describe('pinSession', () => {
    it('pins the active session when target is omitted', async () => {
      await controller.resumeSession('session-alpha-12345678');
      expect(controller.activeSessionId).toBe('session-alpha-12345678');

      const ok = await controller.pinSession(undefined, true);
      expect(ok).toBe(true);
      expect(pinnedCalls).toEqual([{ id: 'session-alpha-12345678', pinned: true }]);
    });

    it('unpins the active session when target is omitted', async () => {
      await controller.resumeSession('session-beta-87654321');
      const ok = await controller.pinSession(undefined, false);
      expect(ok).toBe(true);
      expect(pinnedCalls).toEqual([{ id: 'session-beta-87654321', pinned: false }]);
    });

    it('returns false and emits notice if no active session and target omitted', async () => {
      const ok = await controller.pinSession(undefined, true);
      expect(ok).toBe(false);
      expect(pinnedCalls).toHaveLength(0);
    });

    it('resolves unique session prefix target', async () => {
      const ok = await controller.pinSession('session-beta', true);
      expect(ok).toBe(true);
      expect(pinnedCalls).toEqual([{ id: 'session-beta-87654321', pinned: true }]);
    });

    it('reports ambiguous prefix without mutating', async () => {
      const ok = await controller.pinSession('session-alpha', true);
      expect(ok).toBe(false);
      expect(pinnedCalls).toHaveLength(0);
    });

    it('reports unknown session without mutating', async () => {
      const ok = await controller.pinSession('non-existent', true);
      expect(ok).toBe(false);
      expect(pinnedCalls).toHaveLength(0);
    });
  });

  describe('archiveSession', () => {
    it('archives active session, resetting active sessionId and telemetry', async () => {
      await controller.resumeSession('session-alpha-12345678');
      expect(controller.activeSessionId).toBe('session-alpha-12345678');

      const ok = await controller.archiveSession();
      expect(ok).toBe(true);
      expect(archivedCalls).toEqual(['session-alpha-12345678']);
      expect(controller.activeSessionId).toBeUndefined();
      expect(controller.latestContextTelemetry).toBeUndefined();

      const snapshot = controller.getSnapshot();
      expect(snapshot.rows.some((r) => r.text?.includes('Archived active session'))).toBe(true);
    });

    it('archives non-active session without resetting active session', async () => {
      await controller.resumeSession('session-alpha-12345678');
      const ok = await controller.archiveSession('session-beta');
      expect(ok).toBe(true);
      expect(archivedCalls).toEqual(['session-beta-87654321']);
      expect(controller.activeSessionId).toBe('session-alpha-12345678');
    });

    it('reports notice if no active session and target omitted', async () => {
      const ok = await controller.archiveSession();
      expect(ok).toBe(false);
      expect(archivedCalls).toHaveLength(0);
    });
  });

  describe('restoreSession', () => {
    it('requires an explicit target', async () => {
      const ok = await controller.restoreSession('');
      expect(ok).toBe(false);
      expect(restoredCalls).toHaveLength(0);
    });

    it('resolves archived session target across full catalog (includeArchived: true)', async () => {
      const ok = await controller.restoreSession('session-archived');
      expect(ok).toBe(true);
      expect(restoredCalls).toEqual(['session-archived-99999999']);
    });

    it('reports notice if target does not exist', async () => {
      const ok = await controller.restoreSession('session-missing');
      expect(ok).toBe(false);
      expect(restoredCalls).toHaveLength(0);
    });
  });
});
