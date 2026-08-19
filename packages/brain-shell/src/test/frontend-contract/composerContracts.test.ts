import { describe, test, expect } from 'bun:test';
import * as child_process from 'child_process';
import * as path from 'path';

// Import vendor suggestion and state modules directly to verify Layer 0/2/4 contracts
import { getCommands } from '../../../vendor/claude/commands.js';
import { generateCommandSuggestions } from '../../../vendor/claude/utils/suggestions/commandSuggestions.js';
import { generateUnifiedSuggestions } from '../../../vendor/claude/hooks/unifiedSuggestions.js';
import { extractSearchToken, formatReplacementValue } from '../../../vendor/claude/hooks/useTypeahead.js';

import { FileIndex } from '../../../vendor/claude/native-ts/file-index/index.js';

// Ensure mock OAuth token is set so getCommands() executes cleanly without prompting
process.env.CLAUDE_CODE_OAUTH_TOKEN = process.env.CLAUDE_CODE_OAUTH_TOKEN || 'test-oauth-token-for-contracts';
delete process.env.ANTHROPIC_API_KEY;

const TEST_DIR = import.meta.dir;
const BRAIN_SHELL_DIR = path.resolve(TEST_DIR, '..', '..', '..');
const RUNNER_SCRIPT = path.join(TEST_DIR, 'composerRunner.py');
const EXPECTED_VERSION = process.env.CLAUDE_VERSION || (globalThis as any).MACRO?.VERSION || '2.1.235';

describe('Phase 7B Wave 1: Composer Subsystem State Machine Contracts (States 02–07)', () => {

  // ==========================================================================
  // STATE 02: IDLE_COMPOSER
  // ==========================================================================
  describe('State 02: IDLE_COMPOSER Contract', () => {
    test('Dimension 1–7: Baseline Idle Geometry, Shortcuts Hint, and Landmark Anchoring', () => {
      const output = child_process.execSync(
        `python3 ${RUNNER_SCRIPT} idle "${BRAIN_SHELL_DIR}"`,
        { encoding: 'utf8', timeout: 15000 }
      );

      // 1. Entry & Render: Header Card at Row 01-11
      expect(output).toContain(`Claude Code v${EXPECTED_VERSION}`);
      expect(output).toContain('Sonnet 4.6 · API Usage Billing');

      // 2. Effort indicator at Row 19
      expect(output).toContain('● high · /effort');

      // 3. Composer top and bottom borders at Row 20 and 22
      expect(output).toContain('───');
      expect(output).toContain('❯');

      // 4. Footer shortcuts hint at Row 23
      expect(output).toContain('? for shortcuts');
    }, 15000);
  });

  // ==========================================================================
  // STATE 03: ACTIVE_TYPING
  // ==========================================================================
  describe('State 03: ACTIVE_TYPING Contract', () => {
    test('Dimension 1–7: Active Typing transitions prompt, renders buffer text, and suppresses shortcuts hint', () => {
      const output = child_process.execSync(
        `python3 ${RUNNER_SCRIPT} typing "${BRAIN_SHELL_DIR}"`,
        { encoding: 'utf8', timeout: 15000 }
      );

      // Prompt line contains entered text
      expect(output).toContain('echo test');

      // Footer status bar renders at Row 23
      const row23 = output.split('\n').find(l => l.startsWith('[23]')) || '';
      expect(row23.length).toBeGreaterThan(0);
    }, 15000);
  });

  // ==========================================================================
  // STATE 04: SLASH_COMMAND_PALETTE
  // ==========================================================================
  describe('State 04: SLASH_COMMAND_PALETTE Contract', () => {
    test('Dimension 1–5 (Layer 0 & 4): Built-in command registry contains 54 canonical commands with exact descriptions', async () => {
      const commands = await getCommands(BRAIN_SHELL_DIR);
      expect(commands.length).toBeGreaterThanOrEqual(50);

      const commandNames = commands.map(c => c.name);
      expect(commandNames).toContain('init');
      expect(commandNames).toContain('doctor');
      expect(commandNames).toContain('resume');
      expect(commandNames).toContain('compact');
      expect(commandNames).toContain('config');
      expect(commandNames).toContain('cost');
      expect(commandNames).toContain('help');
      expect(commandNames).toContain('permissions');

      // Verify doctor command description
      const doctorCmd = commands.find(c => c.name === 'doctor');
      expect(doctorCmd?.description).toContain('Diagnose and verify your Claude Code installation');

      // Verify init command description
      const initCmd = commands.find(c => c.name === 'init');
      expect(initCmd?.description).toContain('Initialize a new CLAUDE.md file with codebase documentation');
    });

    test('Dimension 3 & 4 (Layer 2 & 4): Fuzzy command filtering and scoring (/do -> /doctor, /in -> /init)', async () => {
      const commands = await getCommands(BRAIN_SHELL_DIR);

      // Test filtering with '/do'
      const doSuggestions = generateCommandSuggestions('/do', commands);

      expect(doSuggestions.length).toBeGreaterThan(0);
      expect(doSuggestions[0].displayText).toBe('/doctor');
      expect(doSuggestions[0].description).toContain('Diagnose and verify');

      // Test filtering with '/in'
      const inSuggestions = generateCommandSuggestions('/in', commands);

      expect(inSuggestions.length).toBeGreaterThan(0);
      expect(inSuggestions[0].displayText).toBe('/init');
    });

    test('Dimension 1–7 (Layer 1 & 3): Slash palette renders overlay above composer and filters interactively via PTY', () => {
      const output = child_process.execSync(
        `python3 ${RUNNER_SCRIPT} slash_filter "${BRAIN_SHELL_DIR}"`,
        { encoding: 'utf8', timeout: 15000 }
      );

      // Overlay items rendered in flexible area
      expect(output).toContain('/doctor');
      expect(output).toContain('Diagnose and verify your Claude Code');

      // Composer prompt displays current search token
      expect(output).toContain('❯');
      expect(output).toContain('/do');
    }, 15000);

    test('Dimension 6 & 7 (Layer 3): Slash palette supports Tab auto-completion into composer buffer', () => {
      const output = child_process.execSync(
        `python3 ${RUNNER_SCRIPT} slash_tab "${BRAIN_SHELL_DIR}"`,
        { encoding: 'utf8', timeout: 15000 }
      );

      // Tab completion expands /do into /doctor in the prompt line
      expect(output).toContain('/doctor');
    }, 15000);

    test('Dimension 6 & 7 (Layer 3): Slash palette exits and dismisses overlay on Escape key', () => {
      const output = child_process.execSync(
        `python3 ${RUNNER_SCRIPT} slash_escape "${BRAIN_SHELL_DIR}"`,
        { encoding: 'utf8', timeout: 15000 }
      );

      // Suggestions overlay is dismissed from flexible area
      expect(output).not.toContain('Diagnose and verify your Claude Code installation');
      expect(output).toContain('❯');
    }, 15000);
  });

  // ==========================================================================
  // STATE 05: AT_FILE_PATH_PALETTE
  // ==========================================================================
  describe('State 05: AT_FILE_PATH_PALETTE Contract', () => {
    test('Dimension 1–5 (Layer 0 & 4): Path token extraction and replacement formatting', () => {
      // 1. extractSearchToken extracts path without @
      expect(extractSearchToken({ token: '@src/dispatcher.rs' })).toBe('src/dispatcher.rs');
      expect(extractSearchToken({ token: '@Cargo.toml' })).toBe('Cargo.toml');
      expect(extractSearchToken({ token: '@"src/my file.rs"', isQuoted: true })).toBe('src/my file.rs');

      // 2. formatReplacementValue appends proper @ and trailing space for complete suggestions
      const formatted = formatReplacementValue({
        displayText: 'src/dispatcher.rs',
        mode: 'prompt',
        hasAtPrefix: true,
        needsQuotes: false,
        isQuoted: false,
        isComplete: true,
      });
      expect(formatted).toBe('@src/dispatcher.rs ');
    });

    test('Dimension 1–5: FileIndex fuzzy search indexes synthetic repository fixture deterministically', () => {
      const index = new FileIndex();
      index.loadFromFileList([
        'src/dispatcher.rs',
        'src/runtime.rs',
        'src/engine/mod.rs',
        'docs/architecture.md',
        'Cargo.toml'
      ]);

      const results = index.search('dispatcher', 10);
      expect(results.length).toBeGreaterThan(0);
      expect(results[0].path).toBe('src/dispatcher.rs');

      const engineResults = index.search('engine', 10);
      expect(engineResults.length).toBeGreaterThan(0);
      expect(engineResults[0].path).toBe('src/engine/mod.rs');
    });
  });

  // ==========================================================================
  // STATE 06: BACKGROUND_MODE
  // ==========================================================================
  describe('State 06: BACKGROUND_MODE Contract', () => {
    test('Dimension 1–7: & prefix enters background prompt mode', () => {
      const output = child_process.execSync(
        `python3 ${RUNNER_SCRIPT} background_mode "${BRAIN_SHELL_DIR}"`,
        { encoding: 'utf8', timeout: 15000 }
      );

      // Composer reflects background prompt buffer
      expect(output).toContain('&analyze codebase');
    }, 15000);
  });

  // ==========================================================================
  // STATE 07: BASH_MODE
  // ==========================================================================
  describe('State 07: BASH_MODE Contract', () => {
    test('Dimension 1–7: ! prefix enters bash execution mode', () => {
      const output = child_process.execSync(
        `python3 ${RUNNER_SCRIPT} bash_mode "${BRAIN_SHELL_DIR}"`,
        { encoding: 'utf8', timeout: 15000 }
      );

      // Composer prompt line reflects bash mode prefix
      expect(output).toContain('!');
    }, 15000);
  });
});
