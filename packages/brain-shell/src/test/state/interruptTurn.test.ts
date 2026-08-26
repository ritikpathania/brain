/** Inc 24: escape-interrupted turns settle honestly — frozen partial output,
 * an INTERRUPTED_ROW system row, tools cancelled, session still reusable. */
import { describe, it, expect } from 'bun:test';
import { SessionController, INTERRUPTED_ROW } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainGenerationRequest,
  BrainStreamChunk,
} from '../../client/BrainBackendClient.js';

class InterruptibleClient {
  mode: 'cancelled' | 'completed' = 'cancelled';
  createCalls = 0;
  /** Resolves when the controller aborts the request's signal. */
  async createSession(): Promise<{ sessionId: string }> {
    this.createCalls += 1;
    return { sessionId: `sess_${this.createCalls}` };
  }
  async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
    yield { type: 'token', token: 'par' } as BrainStreamChunk;
    const signal = request.signal;
    if (this.mode === 'cancelled' && signal && !signal.aborted) {
      await new Promise<void>((resolve) =>
        signal.addEventListener('abort', () => resolve(), { once: true }),
      );
    }
    yield { type: 'token', token: 'tial' } as BrainStreamChunk;
    yield {
      type: 'finished',
      status: this.mode === 'cancelled' ? 'cancelled' : 'completed',
    } as BrainStreamChunk;
  }
}

function makeController(client: InterruptibleClient): SessionController {
  return new SessionController(client as unknown as BrainBackendClient);
}

async function settle(controller: SessionController): Promise<void> {
  for (let i = 0; i < 400 && controller.getSnapshot().busy; i++) {
    await new Promise((r) => setTimeout(r, 5));
  }
}

describe('Inc 24 graceful interrupt', () => {
  it('interrupting mid-turn settles interrupted with a visible row, not error', async () => {
    const client = new InterruptibleClient();
    const controller = makeController(client);
    const done = controller.submit('hello');
    await new Promise((r) => setTimeout(r, 20)); // let the first token land
    expect(controller.getSnapshot().busy).toBe(true);
    controller.interruptTurn();
    await done;
    await settle(controller);
    const snap = controller.getSnapshot();
    expect(snap.busy).toBe(false);
    expect(snap.rows.some((r) => r.kind === 'system' && r.text === INTERRUPTED_ROW)).toBe(
      true,
    );
  });

  it('idle interruptTurn is a safe no-op', () => {
    const controller = makeController(new InterruptibleClient());
    expect(() => controller.interruptTurn()).not.toThrow();
    expect(controller.getSnapshot().busy).toBe(false);
  });

  it('session stays reusable after an interrupt (next turn completes)', async () => {
    const client = new InterruptibleClient();
    const controller = makeController(client);
    const first = controller.submit('one');
    await new Promise((r) => setTimeout(r, 20));
    controller.interruptTurn();
    await first;
    await settle(controller);
    client.mode = 'completed';
    await controller.submit('two');
    await settle(controller);
    const texts = controller
      .getSnapshot()
      .rows.map((r) => ('text' in r ? r.text : 'markdown' in r ? r.markdown : ''));
    expect(texts.some((t) => t.includes('partial'))).toBe(true);
    // Exactly one interruption row across both turns.
    const interrupts = controller
      .getSnapshot()
      .rows.filter((r) => r.kind === 'system' && r.text === INTERRUPTED_ROW);
    expect(interrupts.length).toBe(1);
  });
});
