import { describe, it, expect } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainStreamChunk,
  BrainGenerationRequest,
} from '../../client/BrainBackendClient.js';

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

interface StubOpts {
  /** Sleep AFTER yielding a token, letting real ticker ticks drain. */
  postTokenSleepMs?: number;
}

function stubSession(): { sessionId: string; title: string; createdAtMs: number } {
  return { sessionId: 'freeze-probe', title: 't', createdAtMs: 0 };
}

function stubClient(
  chunks: BrainStreamChunk[],
  opts: StubOpts = {},
): BrainBackendClient {
  return {
    async createSession() {
      return stubSession();
    },
    async *streamText(_req: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
      for (const c of chunks) {
        yield c;
        if (c.type === 'token' && opts.postTokenSleepMs) await sleep(opts.postTokenSleepMs);
      }
    },
  } as unknown as BrainBackendClient;
}

/** Frozen assistant markdown, joined by '|' if more than one row exists. */
function markdownOf(ctl: SessionController): string {
  return ctl
    .getSnapshot()
    .rows.filter((r) => r.kind === 'assistant')
    .map((r) => r.markdown)
    .join('|');
}

describe('Inc 16: freeze-path renders every delta exactly once', () => {
  it('renders a single instantly-completing token exactly once', async () => {
    const ctl = new SessionController(
      stubClient([{ type: 'token', token: 'abc' }, { type: 'finished', status: 'completed' }]),
    );
    await ctl.submit('q'); // microtasks beat the 16 ms tick: zero drains
    expect(markdownOf(ctl)).toBe('abc');
    ctl.dispose();
  });

  it('renders multi-token instant completion as one exact concatenation', async () => {
    const toks = ['He', 'llo', ' wo', 'rld'];
    const ctl = new SessionController(
      stubClient([
        ...toks.map((token) => ({ type: 'token' as const, token })),
        { type: 'finished', status: 'completed' as const },
      ]),
    );
    await ctl.submit('q');
    expect(markdownOf(ctl)).toBe('Hello world');
    ctl.dispose();
  });

  it('renders a partially drained stream without duplicating its tail', async () => {
    const big = 'y'.repeat(196) + 'TAIL'; // > 32×3 so ticks cannot finish it
    const ctl = new SessionController(
      stubClient(
        [{ type: 'token', token: big }, { type: 'finished', status: 'completed' }],
        { postTokenSleepMs: 50 },
      ),
    );
    await ctl.submit('q');
    expect(markdownOf(ctl)).toBe(big);
    ctl.dispose();
  });

  it('renders error-chunk completion exactly once', async () => {
    const ctl = new SessionController(
      stubClient([
        { type: 'token', token: 'partial' },
        { type: 'error', error: 'v1/generation/stream aborted' }, // abort-classified: no monitor arming
      ]),
    );
    await ctl.submit('q');
    expect(markdownOf(ctl)).toBe('partial');
    ctl.dispose();
  });

  it('renders a real mid-stream abort exactly once', async () => {
    const big = 'z'.repeat(100);
    const ctl = new SessionController({
      async createSession() {
        return stubSession();
      },
      async *streamText(req: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
        yield { type: 'token', token: big };
        while (!req.signal.aborted) await sleep(5);
        throw new Error('The operation was aborted');
      },
    } as unknown as BrainBackendClient);
    const turn = ctl.submit('q');
    await sleep(25); // ~0–2 ticks drain some chars into the discarded live view
    ctl.abort();
    await turn;
    expect(markdownOf(ctl)).toBe(big);
    ctl.dispose();
  });

  it('keeps empty responses and thinking-only turns free of phantom rows', async () => {
    const emptyCtl = new SessionController(
      stubClient([{ type: 'finished', status: 'completed' }]),
    );
    await emptyCtl.submit('q');
    expect(emptyCtl.getSnapshot().rows.filter((r) => r.kind === 'assistant').length).toBe(0);
    emptyCtl.dispose();

    const thinkCtl = new SessionController(
      stubClient([
        { type: 'thinking_start' } as unknown as BrainStreamChunk,
        { type: 'thinking', thinking: 'pondering' } as unknown as BrainStreamChunk,
        { type: 'thinking_end', durationMs: 9 } as unknown as BrainStreamChunk,
        { type: 'finished', status: 'completed' },
      ]),
    );
    await thinkCtl.submit('q');
    expect(markdownOf(thinkCtl)).toBe('');
    const think = thinkCtl.getSnapshot().rows.find((r) => r.kind === 'thinking');
    expect(think?.text).toBe('pondering');
    thinkCtl.dispose();
  });
});
