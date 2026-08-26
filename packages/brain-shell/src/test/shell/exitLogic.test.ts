/** Inc 24: exit classification — busy requests interrupt first, idle exit. */
import { describe, it, expect } from 'bun:test';
import {
  planUserExit,
  planQuit,
  makeUserExit,
} from '../../ui/shell/exitLogic.js';

describe('exitLogic', () => {
  it('idle ctrl+c plans an immediate exit', () => {
    expect(planUserExit(false)).toEqual({ kind: 'exit' });
  });
  it('busy ctrl+c plans an interrupt with a press-again notice', () => {
    const plan = planUserExit(true);
    expect(plan.kind).toBe('interrupt');
    if (plan.kind === 'interrupt') expect(plan.notice).toContain('ctrl+c again');
  });
  it('/quit while busy also plans an interrupt (with quitting copy)', () => {
    const plan = planQuit(true);
    expect(plan.kind).toBe('interrupt');
    if (plan.kind === 'interrupt') expect(plan.notice).toContain('quitt');
  });
  it('makeUserExit interrupts when busy, exits when idle', () => {
    let busy = true;
    const calls: string[] = [];
    const fake = {
      getSnapshot: () => ({ busy }),
      interruptTurn: () => {
        calls.push('interrupt');
        busy = false;
      },
      notice: (t: string) => calls.push(`notice:${t}`),
    };
    let exited = 0;
    const exit = makeUserExit(fake as never, () => {
      exited += 1;
    });
    exit();
    expect(calls[0]).toBe('interrupt');
    expect(String(calls[1])).toContain('notice:');
    expect(exited).toBe(0);
    busy = false;
    exit();
    expect(exited).toBe(1);
  });
});
