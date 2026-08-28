import { describe, expect, it } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import { tokenizeArgs, parseMemoryAddArgs } from '../../commands/memoryArgs.js';
import { getCommand } from '../../commands/registry.js';
import '../../commands/builtin.js';
import type {
  BrainBackendClient,
  MemoryStoreInput,
  MemoryStoreOutput,
} from '../../client/BrainBackendClient.js';

class MockBackendClient implements Partial<BrainBackendClient> {
  public storeCalls: MemoryStoreInput[] = [];
  public shouldFailStore = false;

  async storeMemory(input: MemoryStoreInput): Promise<MemoryStoreOutput> {
    this.storeCalls.push(input);
    if (this.shouldFailStore) {
      throw new Error('Database locked: mock storage error');
    }
    return {
      success: true,
      nodeId: `node_${this.storeCalls.length}_${input.label.toLowerCase().replace(/\s+/g, '_')}`,
    };
  }

  // Stubs for SessionController init
  onChunk(): () => void {
    return () => {};
  }
  async loadSession(): Promise<any> {
    return { session: { id: 'test-session', messages: [] } };
  }
}

describe('tokenizeArgs', () => {
  it('splits whitespace separated words', () => {
    expect(tokenizeArgs('Quantum Teleportation --content photons')).toEqual([
      'Quantum',
      'Teleportation',
      '--content',
      'photons',
    ]);
  });

  it('preserves double-quoted tokens with whitespace', () => {
    expect(
      tokenizeArgs('"Quantum Teleportation" --content "Entangled photon pairs transfer state"'),
    ).toEqual(['Quantum Teleportation', '--content', 'Entangled photon pairs transfer state']);
  });

  it('preserves single-quoted tokens', () => {
    expect(tokenizeArgs("'Superconducting Qubits' --content 'Fluxonium and Transmon'")).toEqual([
      'Superconducting Qubits',
      '--content',
      'Fluxonium and Transmon',
    ]);
  });
});

describe('parseMemoryAddArgs', () => {
  it('parses canonical multi-word label and multi-word --content', () => {
    const args = ['Quantum', 'Teleportation', '--content', 'Entangled', 'photons', 'transfer'];
    const res = parseMemoryAddArgs(args);
    expect(res).toEqual({
      ok: true,
      data: {
        label: 'Quantum Teleportation',
        content: 'Entangled photons transfer',
      },
    });
  });

  it('parses quoted tokens with --content', () => {
    const args = ['Quantum Teleportation', '--content', 'Entangled photons transfer'];
    const res = parseMemoryAddArgs(args);
    expect(res).toEqual({
      ok: true,
      data: {
        label: 'Quantum Teleportation',
        content: 'Entangled photons transfer',
      },
    });
  });

  it('parses single positional token defaulting content to label', () => {
    const args = ['Superconductors'];
    const res = parseMemoryAddArgs(args);
    expect(res).toEqual({
      ok: true,
      data: {
        label: 'Superconductors',
        content: 'Superconductors',
      },
    });
  });

  it('parses positional multiple tokens: first token is label, remainder is content', () => {
    const args = ['Superconductors', 'Zero', 'electrical', 'resistance'];
    const res = parseMemoryAddArgs(args);
    expect(res).toEqual({
      ok: true,
      data: {
        label: 'Superconductors',
        content: 'Zero electrical resistance',
      },
    });
  });

  it('returns error when --content is specified with empty label', () => {
    const args = ['--content', 'Entangled photons'];
    const res = parseMemoryAddArgs(args);
    expect(res.ok).toBe(false);
    if (!res.ok) {
      expect(res.error).toContain('Memory node requires a non-empty label');
    }
  });

  it('returns error when --content is specified with empty content', () => {
    const args = ['Quantum Teleportation', '--content'];
    const res = parseMemoryAddArgs(args);
    expect(res.ok).toBe(false);
    if (!res.ok) {
      expect(res.error).toContain('Memory node requires non-empty content');
    }
  });

  it('returns error on empty args', () => {
    const res = parseMemoryAddArgs([]);
    expect(res.ok).toBe(false);
    if (!res.ok) {
      expect(res.error).toContain('Usage: /memory add');
    }
  });
});

describe('SessionController.storeMemoryNode', () => {
  it('stores concept node and emits confirmation notice', async () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    const res = await controller.storeMemoryNode(
      'Quantum Teleportation',
      'Entangled photons transfer quantum state',
    );

    expect(res).toBeDefined();
    expect(res?.success).toBe(true);
    expect(res?.nodeId).toContain('quantum_teleportation');
    expect(mock.storeCalls.length).toBe(1);
    expect(mock.storeCalls[0]?.label).toBe('Quantum Teleportation');
    expect(mock.storeCalls[0]?.content).toBe('Entangled photons transfer quantum state');

    const lastRow = controller.getSnapshot().rows.slice(-1)[0];
    expect(lastRow?.kind).toBe('system');
    expect((lastRow as any)?.text).toContain('Stored memory concept “Quantum Teleportation”');
  });

  it('rejects empty label without calling client', async () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    const res = await controller.storeMemoryNode('   ', 'Some content');
    expect(res).toBeUndefined();
    expect(mock.storeCalls.length).toBe(0);

    const lastRow = controller.getSnapshot().rows.slice(-1)[0];
    expect((lastRow as any)?.text).toContain('Memory node requires a non-empty label.');
  });

  it('rejects empty content without calling client', async () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    const res = await controller.storeMemoryNode('My Concept', '   ');
    expect(res).toBeUndefined();
    expect(mock.storeCalls.length).toBe(0);

    const lastRow = controller.getSnapshot().rows.slice(-1)[0];
    expect((lastRow as any)?.text).toContain('Memory node requires non-empty content.');
  });

  it('handles backend error gracefully without mutating state', async () => {
    const mock = new MockBackendClient();
    mock.shouldFailStore = true;
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    const res = await controller.storeMemoryNode('Failing Concept', 'Will fail in storage');
    expect(res).toBeUndefined();

    const lastRow = controller.getSnapshot().rows.slice(-1)[0];
    expect((lastRow as any)?.text).toContain('Could not store memory node: Database locked');
  });
});

describe('/memory command routing', () => {
  const memoryCmd = getCommand('memory')!;

  it('returns overlay for bare /memory', () => {
    const res = memoryCmd.run({ args: [], raw: '/memory', isBash: false });
    expect(res).toEqual({ type: 'overlay', overlay: 'memory' });
  });

  it('returns action for /memory delete <id>', () => {
    const res = memoryCmd.run({ args: ['delete', 'node_123'], raw: '/memory delete node_123', isBash: false });
    expect(res).toEqual({ type: 'action', action: 'memory-delete', args: ['node_123'] });
  });

  it('returns action for /memory add <label> --content <text>', () => {
    const res = memoryCmd.run({
      args: ['add', 'Quantum', 'Teleportation', '--content', 'Entangled', 'photons'],
      raw: '/memory add Quantum Teleportation --content Entangled photons',
      isBash: false,
    });
    expect(res).toEqual({
      type: 'action',
      action: 'memory-add',
      args: ['Quantum Teleportation', 'Entangled photons'],
    });
  });

  it('returns usage text when /memory add is called with invalid args', () => {
    const res = memoryCmd.run({
      args: ['add', '--content', 'Only content'],
      raw: '/memory add --content Only content',
      isBash: false,
    });
    expect(res.type).toBe('text');
    expect((res as any).value).toContain('Memory node requires a non-empty label');
  });
});
