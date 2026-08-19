import { describe, test, expect } from 'bun:test';
import * as child_process from 'child_process';
import * as path from 'path';

// Import vendor modules directly to verify Layer 0 & Layer 4 contracts
import { getModelOptions } from '../../../vendor/claude/utils/model/modelOptions.js';
import { getFilePermissionOptions } from '../../../vendor/claude/components/permissions/FilePermissionDialog/permissionOptions.js';
import { bashToolUseOptions } from '../../../vendor/claude/components/permissions/BashPermissionRequest/bashToolUseOptions.js';
import { convertEffortValueToLevel } from '../../../vendor/claude/utils/effort.js';

// Ensure mock OAuth token is set
process.env.CLAUDE_CODE_OAUTH_TOKEN = process.env.CLAUDE_CODE_OAUTH_TOKEN || 'test-oauth-token-for-overlays';
delete process.env.ANTHROPIC_API_KEY;

const TEST_DIR = import.meta.dir;
const BRAIN_SHELL_DIR = path.resolve(TEST_DIR, '..', '..', '..');
const OVERLAY_RUNNER = path.join(TEST_DIR, 'overlayRunner.py');

const CANONICAL_VIEWPORTS = [
  { name: '80x24 (Standard Compact)', cols: 80, rows: 24 },
  { name: '100x26 (Medium Desktop)', cols: 100, rows: 26 },
  { name: '120x30 (Widescreen Terminal)', cols: 120, rows: 30 },
  { name: '182x53 (Fullscreen Display)', cols: 182, rows: 53 },
];

describe('Phase 7B Wave 2: Overlays & Dialogs State Machine Contracts (States 09–11)', () => {

  // ==========================================================================
  // STATE 09: HELP_MENU_OVERLAY
  // ==========================================================================
  describe('State 09: HELP_MENU_OVERLAY Contract', () => {
    test('Dimension 1–5 (Layer 0 & 4): Help overlay invoked via "?" renders exact 3-column vendor layout', () => {
      const output = child_process.execSync(
        `python3 ${OVERLAY_RUNNER} help_question "${BRAIN_SHELL_DIR}" 80 24`,
        { encoding: 'utf8', timeout: 15000 }
      );

      // Column 1: Prefix modes
      expect(output).toContain('! for bash mode');
      expect(output).toContain('/ for commands');
      expect(output).toContain('@ for file paths');
      expect(output).toContain('& for background');
      expect(output).toContain('/btw for side');

      // Column 2: Common actions (exact vendor strings)
      expect(output).toContain('double tap esc to clear input');
      expect(output).toContain('shift + tab to auto-accept');
      expect(output).toContain('ctrl + o for verbose output');
      expect(output).toContain('ctrl + t to toggle tasks');

      // Column 3: Editing & navigation
      expect(output).toContain('ctrl + s to stash');
      expect(output).toContain('ctrl + g to edit in');
      expect(output).toContain('$EDITOR');
    }, 15000);

    test('Dimension 6 & 7 (Layer 3): Help overlay dismisses cleanly on Escape without prompt leakage', () => {
      const output = child_process.execSync(
        `python3 ${OVERLAY_RUNNER} help_escape "${BRAIN_SHELL_DIR}" 80 24`,
        { encoding: 'utf8', timeout: 15000 }
      );

      // Help menu is dismissed
      expect(output).not.toContain('! for bash mode');
      // Prompt restored to baseline
      expect(output).toContain('❯');
    }, 15000);

    for (const vp of CANONICAL_VIEWPORTS) {
      test(`Dimension 4 (Layer 1): Help overlay multi-viewport cell rendering across ${vp.name}`, () => {
        const output = child_process.execSync(
          `python3 ${OVERLAY_RUNNER} help_question "${BRAIN_SHELL_DIR}" ${vp.cols} ${vp.rows}`,
          { encoding: 'utf8', timeout: 15000 }
        );

        const EXPECTED_VERSION = process.env.CLAUDE_VERSION || (globalThis as any).MACRO?.VERSION || '2.1.235';
        expect(output).toContain('! for bash mode');
        expect(output).toContain('/ for commands');
        expect(output).toContain(`Claude Code v${EXPECTED_VERSION}`);
      }, 15000);
    }
  });

  // ==========================================================================
  // STATE 10: MODEL_PICKER_MODAL
  // ==========================================================================
  describe('State 10: MODEL_PICKER_MODAL Contract', () => {
    test('Dimension 1–5 (Layer 0 & 4): Model catalog is dynamically derived from vendor getModelOptions', () => {
      const options = getModelOptions(false);
      expect(options.length).toBeGreaterThanOrEqual(3);

      const labels = options.map(o => o.label);
      expect(labels).toContain('Default (recommended)');
      expect(labels).toContain('Opus');
      expect(labels).toContain('Haiku');

      // Verify effort conversion and description contract
      expect(convertEffortValueToLevel('low')).toBe('low');
      expect(convertEffortValueToLevel('medium')).toBe('medium');
      expect(convertEffortValueToLevel('high')).toBe('high');
      expect(convertEffortValueToLevel('max')).toBe('max');
    });

    test('Dimension 1–7 (Layer 1, 3, 4): Model picker invoked via alt+p displays exact catalog and effort controls', () => {
      const output = child_process.execSync(
        `python3 ${OVERLAY_RUNNER} model_picker_alt_p "${BRAIN_SHELL_DIR}" 80 24`,
        { encoding: 'utf8', timeout: 15000 }
      );

      // Model picker banner
      expect(output).toContain('Switch between Claude models');
      expect(output).toContain('Default (recommended)');
      expect(output).toContain('Sonnet 4.6');

      // Effort adjustment controls
      expect(output).toContain('High effort');
      expect(output).toContain('Enter to confirm · Esc to exit');
    }, 15000);

    test('Dimension 6 & 7 (Layer 3): Model picker dismisses cleanly on Escape without state mutation', () => {
      const output = child_process.execSync(
        `python3 ${OVERLAY_RUNNER} model_picker_escape "${BRAIN_SHELL_DIR}" 80 24`,
        { encoding: 'utf8', timeout: 15000 }
      );

      expect(output).not.toContain('Switch between Claude models');
      expect(output).toContain('Sonnet 4.6 · API Usage Billing');
    }, 15000);

    for (const vp of CANONICAL_VIEWPORTS) {
      test(`Dimension 4 (Layer 1): Model picker multi-viewport cell rendering across ${vp.name}`, () => {
        const output = child_process.execSync(
          `python3 ${OVERLAY_RUNNER} model_picker_alt_p "${BRAIN_SHELL_DIR}" ${vp.cols} ${vp.rows}`,
          { encoding: 'utf8', timeout: 15000 }
        );

        expect(output).toContain('Switch between Claude models');
        expect(output).toContain('Enter to confirm');
      }, 15000);
    }
  });

  // ==========================================================================
  // STATE 11: PERMISSION_REQUEST_DIALOG
  // ==========================================================================
  describe('State 11: PERMISSION_REQUEST_DIALOG Contract', () => {
    test('Dimension 1–5 (Layer 0 & 4): File Permission options match vendor reference contract', () => {
      const mockContext = {
        additionalWorkingDirectories: new Map(),
        alwaysAllowRules: {},
      } as any;

      const options = getFilePermissionOptions({
        filePath: path.join(BRAIN_SHELL_DIR, 'test.ts'),
        toolPermissionContext: mockContext,
        operationType: 'write',
      });

      expect(options.length).toBeGreaterThanOrEqual(2);
      const values = options.map(o => o.value);
      expect(values).toContain('yes');
      expect(values).toContain('no');
    });

    test('Dimension 1–5 (Layer 0 & 4): Bash Permission options match vendor reference contract', () => {
      const options = bashToolUseOptions({
        command: 'cargo build',
        description: 'Build rust project',
        onAcceptFeedbackChange: () => {},
        onRejectFeedbackChange: () => {},
      });

      expect(options.length).toBeGreaterThanOrEqual(2);
      const values = options.map(o => o.value);
      expect(values).toContain('yes');
      expect(values).toContain('no');
    });
  });
});
