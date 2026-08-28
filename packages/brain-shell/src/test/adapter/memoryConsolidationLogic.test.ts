import { describe, expect, it } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import { getCommand } from '../../commands/registry.js';
import '../../commands/builtin.js';
import type {
  BrainBackendClient,
  ConsolidationResult,
} from '../../client/BrainBackendClient.js';

class MockBackendClient implements Partial<BrainBackendClient> {
  public consolidateOptions?: {
    promotionWeightThreshold?: number;
    pruningWeightThreshold?: number;
    stalenessAgeThresholdSecs?: number;
  };
  public mockResult: ConsolidationResult = {
    actionsApplied: 0,
    promoted: 0,
    merged: 0,
    archived: 0,
    pruned: 0,
    durationMs: 4,
    errors: [],
  };
  public shouldFail = false;

  async triggerConsolidation(options?: {
    promotionWeightThreshold?: number;
    pruningWeightThreshold?: number;
    stalenessAgeThresholdSecs?: number;
  }): Promise<ConsolidationResult> {
    this.consolidateOptions = options;
    if (this.shouldFail) {
      throw new Error('Database locked: lock acquisition timeout');
    }
    return this.mockResult;
  }

  // Stubs for SessionController init
  onChunk(): () => void {
    return () => {};
  }
  async loadSession(): Promise<any> {
    return { session: { id: 'test-session', messages: [] } };
  }
}

describe('SessionController.consolidateMemories', () => {
  it('emits fully-consolidated notice when 0 actions were needed', async () => {
    const mock = new MockBackendClient();
    mock.mockResult = {
      actionsApplied: 0,
      promoted: 0,
      merged: 0,
      archived: 0,
      pruned: 0,
      durationMs: 3,
      errors: [],
    };
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    const res = await controller.consolidateMemories();
    expect(res).toEqual(mock.mockResult);

    const lastRow = controller.getSnapshot().rows.slice(-1)[0];
    expect(lastRow?.kind).toBe('system');
    expect((lastRow as any)?.text).toContain('Memory graph is fully consolidated: 0 actions needed in 3ms');
  });

  it('emits structured actions notice when actions were applied', async () => {
    const mock = new MockBackendClient();
    mock.mockResult = {
      actionsApplied: 4,
      promoted: 2,
      merged: 1,
      archived: 1,
      pruned: 0,
      durationMs: 18,
      errors: [],
    };
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    const res = await controller.consolidateMemories({
      promotionWeightThreshold: 0.8,
      pruningWeightThreshold: 0.2,
      stalenessAgeThresholdSecs: 3600,
    });
    expect(res).toEqual(mock.mockResult);
    expect(mock.consolidateOptions?.promotionWeightThreshold).toBe(0.8);

    const lastRow = controller.getSnapshot().rows.slice(-1)[0];
    expect(lastRow?.kind).toBe('system');
    expect((lastRow as any)?.text).toContain(
      'Consolidated memory graph: 4 actions applied (2 promoted, 1 merged, 1 archived, 0 pruned) in 18ms',
    );
  });

  it('surfaces backend errors truthfully when result contains errors', async () => {
    const mock = new MockBackendClient();
    mock.mockResult = {
      actionsApplied: 1,
      promoted: 0,
      merged: 0,
      archived: 0,
      pruned: 0,
      durationMs: 5,
      errors: ['Failed to merge node A into B: FK constraint violation'],
    };
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    const res = await controller.consolidateMemories();
    expect(res).toEqual(mock.mockResult);

    const lastRow = controller.getSnapshot().rows.slice(-1)[0];
    expect(lastRow?.kind).toBe('system');
    expect((lastRow as any)?.text).toContain(
      'Memory consolidation completed with errors: Failed to merge node A into B: FK constraint violation',
    );
  });

  it('handles RPC rejection without mutating state', async () => {
    const mock = new MockBackendClient();
    mock.shouldFail = true;
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    const res = await controller.consolidateMemories();
    expect(res).toBeUndefined();

    const lastRow = controller.getSnapshot().rows.slice(-1)[0];
    expect(lastRow?.kind).toBe('system');
    expect((lastRow as any)?.text).toContain('Could not consolidate memory graph: Database locked');
  });
});

describe('/memory consolidate and /consolidate command routing', () => {
  it('routes /memory consolidate to memory-consolidate action', () => {
    const memoryCmd = getCommand('memory')!;
    const res = memoryCmd.run({
      args: ['consolidate'],
      raw: '/memory consolidate',
      isBash: false,
    });
    expect(res).toEqual({ type: 'action', action: 'memory-consolidate' });
  });

  it('routes top-level /consolidate command to memory-consolidate action', () => {
    const consolidateCmd = getCommand('consolidate')!;
    expect(consolidateCmd).toBeDefined();
    const res = consolidateCmd.run({
      args: [],
      raw: '/consolidate',
      isBash: false,
    });
    expect(res).toEqual({ type: 'action', action: 'memory-consolidate' });
  });
});
