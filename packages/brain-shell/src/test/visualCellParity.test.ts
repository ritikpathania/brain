import { describe, test, expect } from 'bun:test';
import * as child_process from 'child_process';
import * as path from 'path';

const TEST_DIR = import.meta.dir;
const PARITY_SCRIPT = path.join(TEST_DIR, 'testDeterministicParity.py');
const FORENSIC_SCRIPT = path.join(TEST_DIR, 'forensicCellDiff.py');

describe('Phase 6.5: Terminal 80x24 Cell Grid Layout & Visual Parity Suite', () => {
  test('Visual Invariant 1: FullscreenLayout activates bottom-anchored composer and flexible viewport', () => {
    const output = child_process.execSync(`python3 ${FORENSIC_SCRIPT}`, {
      encoding: 'utf8',
      env: {
        ...process.env,
        CLAUDE_CODE_NO_FLICKER: '1',
      },
    });

    // Verify critical layout landmarks exist at exact rows
    expect(output).toContain('Row 00 MATCH');
    expect(output).toContain('Row 01 MATCH');
    expect(output).toContain('Row 03 MATCH');
    expect(output).toContain('Row 04 MATCH');
    expect(output).toContain('Row 06 MATCH');
    expect(output).toContain('Row 07 MATCH');
    expect(output).toContain('Row 08 MATCH');
    expect(output).toContain('Row 09 MATCH');
    expect(output).toContain('Row 10 MATCH');
    expect(output).toContain('Row 11 MATCH');
    expect(output).toContain('Row 12 MATCH');
    expect(output).toContain('Row 14 MATCH');
    expect(output).toContain('Row 15 MATCH');
    expect(output).toContain('Row 16 MATCH');
    expect(output).toContain('Row 17 MATCH');
    expect(output).toContain('Row 18 MATCH');
    expect(output).toContain('Row 20 MATCH');
    expect(output).toContain('Row 22 MATCH');
  }, 15000);

  test('Visual Invariant 2: Row-by-row structure under controlled deterministic state', () => {
    const output = child_process.execSync(`python3 ${PARITY_SCRIPT}`, {
      encoding: 'utf8',
      env: {
        ...process.env,
        CLAUDE_CODE_NO_FLICKER: '1',
      },
    });

    // In controlled state, card frame matches exactly
    expect(output).toContain('Row 00 MATCH');
    expect(output).toContain('Row 01 MATCH');
    expect(output).toContain('Row 02 MATCH');
    expect(output).toContain('Row 03 MATCH');
    expect(output).toContain('Row 04 MATCH');
    expect(output).toContain('Row 05 MATCH');
    expect(output).toContain('Row 08 MATCH');
    expect(output).toContain('Row 09 MATCH');
    expect(output).toContain('Row 10 MATCH');
    expect(output).toContain('Row 11 MATCH');
    expect(output).toContain('Row 20 MATCH');
    expect(output).toContain('Row 21 MATCH');
    expect(output).toContain('Row 22 MATCH');
  }, 15000);
});
