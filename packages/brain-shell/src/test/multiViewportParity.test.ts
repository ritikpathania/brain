import { describe, test, expect } from 'bun:test';
import * as child_process from 'child_process';
import * as path from 'path';

const TEST_DIR = import.meta.dir;
const TARGET_DIR = path.resolve(TEST_DIR, '..', '..');
const RUNNER_SCRIPT = path.join(TEST_DIR, 'multiViewportParityRunner.py');

const VIEWPORTS = [
  { cols: 80, rows: 24, name: '80x24 Standard Compact' },
  { cols: 100, rows: 26, name: '100x26 Medium Desktop' },
  { cols: 120, rows: 30, name: '120x30 Widescreen Terminal' },
  { cols: 182, rows: 53, name: '182x53 Fullscreen Display' },
];

describe('Phase 6.5 Final Hardening Gate: Multi-Viewport Deterministic Parity Matrix', () => {
  for (const vp of VIEWPORTS) {
    test(`Viewport ${vp.cols}x${vp.rows} (${vp.name}) — Empty Composer State`, () => {
      const cmd = `python3 ${RUNNER_SCRIPT} ${vp.cols} ${vp.rows} "${TARGET_DIR}" ""`;
      const output = child_process.execSync(cmd, { encoding: 'utf8' });
      expect(output).toContain(`VERIFIED ${vp.cols}x${vp.rows} empty_composer: ${vp.cols * vp.rows} cells`);
    }, 30000);

    test(`Viewport ${vp.cols}x${vp.rows} (${vp.name}) — Active Composer State ('Hello world')`, () => {
      const cmd = `python3 ${RUNNER_SCRIPT} ${vp.cols} ${vp.rows} "${TARGET_DIR}" "Hello world"`;
      const output = child_process.execSync(cmd, { encoding: 'utf8' });
      expect(output).toContain(`VERIFIED ${vp.cols}x${vp.rows} active_composer: ${vp.cols * vp.rows} cells`);
    }, 30000);
  }
});
