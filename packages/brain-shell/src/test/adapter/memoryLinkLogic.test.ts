import { describe, it, expect, vi, beforeEach } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import { MockBrainBackendClient } from '../../client/BrainBackendClient.js';
import { getCommand } from '../../commands/registry.js';
import '../../commands/builtin.js';

describe('Increment 43: Knowledge Graph Relational Authoring Logic', () => {
  let client: MockBrainBackendClient;
  let controller: SessionController;

  beforeEach(() => {
    client = new MockBrainBackendClient();
    controller = new SessionController(client);

    // Mock searchMemory to return predictable nodes
    client.searchMemory = vi.fn().mockResolvedValue({
      memories: [
        {
          id: 'node-auth-12345678',
          label: 'AuthService',
          excerpt: 'Handles user authentication and JWT validation',
          score: 100,
          channel: 'knowledge_graph',
          timestamp: 1000,
          scope: 'workspace',
          relations: [],
        },
        {
          id: 'node-db-87654321',
          label: 'Database',
          excerpt: 'Primary PostgreSQL persistence layer',
          score: 100,
          channel: 'knowledge_graph',
          timestamp: 1000,
          scope: 'workspace',
          relations: [],
        },
        {
          id: 'node-cache-11111111',
          label: 'AuthCache',
          excerpt: 'Redis cache for auth tokens',
          score: 100,
          channel: 'knowledge_graph',
          timestamp: 1000,
          scope: 'workspace',
          relations: [],
        },
        {
          id: 'node-ambig-22222222',
          label: 'CommonService',
          excerpt: 'Common service duplicate 1',
          score: 100,
          channel: 'knowledge_graph',
          timestamp: 1000,
          scope: 'workspace',
          relations: [],
        },
        {
          id: 'node-ambig-33333333',
          label: 'CommonService',
          excerpt: 'Common service duplicate 2',
          score: 100,
          channel: 'knowledge_graph',
          timestamp: 1000,
          scope: 'workspace',
          relations: [],
        },
      ],
      provenance: { count: 5, sources: [], channels: [], epoch_id: 'e1' },
      tokenCount: 100,
      serializedContext: '',
    });
  });

  describe('linkMemories', () => {
    it('links concepts successfully by label with default associated_with relation', async () => {
      const linkSpy = vi.spyOn(client, 'linkMemory');
      const ok = await controller.linkMemories('AuthService', 'Database');

      expect(ok).toBe(true);
      expect(linkSpy).toHaveBeenCalledWith({
        sourceNodeId: 'node-auth-12345678',
        targetNodeId: 'node-db-87654321',
        relation: 'associated_with',
        weight: 1.0,
      });

      const lastRow = controller.getSnapshot().rows.at(-1);
      expect(lastRow?.kind).toBe('system');
      if (lastRow?.kind === 'system') {
        expect(lastRow.text).toContain('Linked “AuthService” → [associated_with] → “Database”');
      }
    });

    it('links concepts with custom relation kind (e.g. depends_on)', async () => {
      const linkSpy = vi.spyOn(client, 'linkMemory');
      const ok = await controller.linkMemories('AuthService', 'Database', 'depends_on');

      expect(ok).toBe(true);
      expect(linkSpy).toHaveBeenCalledWith({
        sourceNodeId: 'node-auth-12345678',
        targetNodeId: 'node-db-87654321',
        relation: 'depends_on',
        weight: 1.0,
      });

      const lastRow = controller.getSnapshot().rows.at(-1);
      if (lastRow?.kind === 'system') {
        expect(lastRow.text).toContain('Linked “AuthService” → [depends_on] → “Database”');
      }
    });

    it('links concepts directly using exact node IDs', async () => {
      const linkSpy = vi.spyOn(client, 'linkMemory');
      const ok = await controller.linkMemories('node-auth-12345678', 'node-cache-11111111', 'uses');

      expect(ok).toBe(true);
      expect(linkSpy).toHaveBeenCalledWith({
        sourceNodeId: 'node-auth-12345678',
        targetNodeId: 'node-cache-11111111',
        relation: 'uses',
        weight: 1.0,
      });
    });

    it('rejects unresolved source node without calling RPC', async () => {
      const linkSpy = vi.spyOn(client, 'linkMemory');
      const ok = await controller.linkMemories('NonExistentService', 'Database');

      expect(ok).toBe(false);
      expect(linkSpy).not.toHaveBeenCalled();

      const lastRow = controller.getSnapshot().rows.at(-1);
      if (lastRow?.kind === 'system') {
        expect(lastRow.text).toContain("Source concept 'NonExistentService' not found in memory");
      }
    });

    it('rejects ambiguous query match without calling RPC', async () => {
      const linkSpy = vi.spyOn(client, 'linkMemory');
      const ok = await controller.linkMemories('CommonService', 'Database');

      expect(ok).toBe(false);
      expect(linkSpy).not.toHaveBeenCalled();

      const lastRow = controller.getSnapshot().rows.at(-1);
      if (lastRow?.kind === 'system') {
        expect(lastRow.text).toContain("Source concept query 'CommonService' is ambiguous");
      }
    });

    it('rejects self-link', async () => {
      const linkSpy = vi.spyOn(client, 'linkMemory');
      const ok = await controller.linkMemories('AuthService', 'AuthService');

      expect(ok).toBe(false);
      expect(linkSpy).not.toHaveBeenCalled();

      const lastRow = controller.getSnapshot().rows.at(-1);
      if (lastRow?.kind === 'system') {
        expect(lastRow.text).toContain('Cannot link a concept node to itself');
      }
    });

    it('handles client RPC rejection with error notice', async () => {
      vi.spyOn(client, 'linkMemory').mockRejectedValue(new Error('UDS connection error'));
      const ok = await controller.linkMemories('AuthService', 'Database');

      expect(ok).toBe(false);
      const lastRow = controller.getSnapshot().rows.at(-1);
      if (lastRow?.kind === 'system') {
        expect(lastRow.text).toContain('Could not link memory concepts: UDS connection error');
      }
    });
  });

  describe('unlinkMemories', () => {
    it('unlinks concepts successfully by label', async () => {
      const unlinkSpy = vi.spyOn(client, 'unlinkMemory');
      const ok = await controller.unlinkMemories('AuthService', 'Database', 'depends_on');

      expect(ok).toBe(true);
      expect(unlinkSpy).toHaveBeenCalledWith({
        sourceNodeId: 'node-auth-12345678',
        targetNodeId: 'node-db-87654321',
        relation: 'depends_on',
      });

      const lastRow = controller.getSnapshot().rows.at(-1);
      if (lastRow?.kind === 'system') {
        expect(lastRow.text).toContain('Unlinked “AuthService” ✕ [depends_on] ✕ “Database”');
      }
    });

    it('unlinks concepts by ID with default associated_with', async () => {
      const unlinkSpy = vi.spyOn(client, 'unlinkMemory');
      const ok = await controller.unlinkMemories('node-auth-12345678', 'node-db-87654321');

      expect(ok).toBe(true);
      expect(unlinkSpy).toHaveBeenCalledWith({
        sourceNodeId: 'node-auth-12345678',
        targetNodeId: 'node-db-87654321',
        relation: 'associated_with',
      });
    });

    it('rejects unresolvable target on unlink', async () => {
      const unlinkSpy = vi.spyOn(client, 'unlinkMemory');
      const ok = await controller.unlinkMemories('AuthService', 'UnknownTarget');

      expect(ok).toBe(false);
      expect(unlinkSpy).not.toHaveBeenCalled();

      const lastRow = controller.getSnapshot().rows.at(-1);
      if (lastRow?.kind === 'system') {
        expect(lastRow.text).toContain("Target concept 'UnknownTarget' not found in memory");
      }
    });
  });

  describe('Command Surface: /memory link & /memory unlink', () => {
    it('parses /memory link <src> <tgt> [rel]', () => {
      const cmd = getCommand('memory');
      expect(cmd).toBeDefined();

      const res1 = cmd!.run({ args: ['link', 'Auth', 'DB'] });
      expect(res1).toEqual({
        type: 'action',
        action: 'memory-link',
        args: ['Auth', 'DB', 'associated_with'],
      });

      const res2 = cmd!.run({ args: ['link', 'Auth', 'DB', 'depends_on'] });
      expect(res2).toEqual({
        type: 'action',
        action: 'memory-link',
        args: ['Auth', 'DB', 'depends_on'],
      });
    });

    it('parses /memory unlink <src> <tgt> [rel]', () => {
      const cmd = getCommand('memory');
      const res = cmd!.run({ args: ['unlink', 'Auth', 'DB', 'depends_on'] });
      expect(res).toEqual({
        type: 'action',
        action: 'memory-unlink',
        args: ['Auth', 'DB', 'depends_on'],
      });
    });

    it('parses top-level /link and /unlink commands', () => {
      const linkCmd = getCommand('link');
      const unlinkCmd = getCommand('unlink');
      expect(linkCmd).toBeDefined();
      expect(unlinkCmd).toBeDefined();

      expect(linkCmd!.run({ args: ['A', 'B'] })).toEqual({
        type: 'action',
        action: 'memory-link',
        args: ['A', 'B', 'associated_with'],
      });

      expect(unlinkCmd!.run({ args: ['A', 'B', 'uses'] })).toEqual({
        type: 'action',
        action: 'memory-unlink',
        args: ['A', 'B', 'uses'],
      });
    });
  });
});
