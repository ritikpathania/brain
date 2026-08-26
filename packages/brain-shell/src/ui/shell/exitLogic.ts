/** Pure classification for exit requests: while a turn streams, ctrl+c and
 * /quit interrupt the turn first; idle requests exit immediately. Mirrors
 * the repo's pure *Logic module convention so the decision stays testable
 * without rendering Ink. */
import type { SessionController } from '../../state/sessionController.js';

export type ExitPlan = { kind: 'exit' } | { kind: 'interrupt'; notice: string };

/** ctrl+c: busy users get their turn back with a hint; idle exits. */
export function planUserExit(busy: boolean): ExitPlan {
  return busy
    ? { kind: 'interrupt', notice: 'Interrupted — press ctrl+c again to exit.' }
    : { kind: 'exit' };
}

/** /quit always quits — politely: tear down an active turn first. */
export function planQuit(busy: boolean): ExitPlan {
  return busy ? { kind: 'interrupt', notice: 'Interrupted — quitting…' } : { kind: 'exit' };
}

/** Binds the ctrl+c decision to a controller. `exit` is injectable so the
 * idle branch stays testable; production passes `process.exit`. */
export function makeUserExit(
  controller: SessionController,
  exit: () => void = () => process.exit(0),
): () => void {
  return () => {
    const plan = planUserExit(controller.getSnapshot().busy);
    if (plan.kind === 'interrupt') {
      controller.interruptTurn();
      controller.notice(plan.notice);
    } else {
      exit();
    }
  };
}
