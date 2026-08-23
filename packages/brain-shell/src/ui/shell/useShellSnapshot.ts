import { useSyncExternalStore } from 'react';
import type { SessionController, ShellSnapshot } from '../../state/sessionController.js';

export function useShellSnapshot(controller: SessionController): ShellSnapshot {
  return useSyncExternalStore(controller.subscribe, controller.getSnapshot);
}
