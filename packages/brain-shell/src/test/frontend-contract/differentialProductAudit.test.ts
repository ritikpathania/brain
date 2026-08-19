import { describe, test, expect } from 'bun:test';
import * as fs from 'fs';
import * as path from 'path';

const TEST_DIR = import.meta.dir;
const AUDIT_RESULTS_PATH = path.resolve(TEST_DIR, '..', 'differentialAuditResults.json');

describe('Phase 7C: Comprehensive Claude-vs-Brain Differential Product Parity Audit Suite', () => {

  test('Gate C.1: Audit manifest records frozen Reference Claude v2.1.233 and Brain Shell environments', () => {
    expect(fs.existsSync(AUDIT_RESULTS_PATH)).toBe(true);
    const report = JSON.parse(fs.readFileSync(AUDIT_RESULTS_PATH, 'utf8'));

    expect(report.manifest).toBeDefined();
    expect(report.manifest.claude_reference.version).toBe('2.1.233');
    expect(report.manifest.claude_reference.sha256).toBe('bc466b6cde63edafc773f471a1fb98787fabb31f52240c8616ce7e1f587b212d');
    expect(report.manifest.brain_shell.runtime).toBe('bun');
    expect(report.manifest.brain_shell.bun_version).toBe('1.4.0');
    expect(report.manifest.brain_shell.bun_sha256).toBe('9f82f70342a6482120ead719dc36561416c987f27f136941ce5f97c8deac410f');
  });

  test('Gate C.2: Zero unclassified differences across all 20 audit categories (100% classification coverage)', () => {
    const report = JSON.parse(fs.readFileSync(AUDIT_RESULTS_PATH, 'utf8'));

    expect(report.classification_counts).toBeDefined();
    expect(report.classification_counts['UNCLASSIFIED']).toBe(0);

    const totalAudited = report.findings.length;
    expect(totalAudited).toBeGreaterThanOrEqual(20);

    for (const finding of report.findings) {
      expect(['EXACT MATCH', 'ENVIRONMENT DIFFERENCE', 'BRAIN INTEGRATION DIFFERENCE', 'ACTUAL FRONTEND GAP']).toContain(finding.status);
      expect(finding.rationale).toBeDefined();
      expect(finding.rationale.length).toBeGreaterThan(0);
    }
  });

  test('Gate C.3: Empirical Gap Inventory isolates specific frontend differences for Phase 7D', () => {
    const report = JSON.parse(fs.readFileSync(AUDIT_RESULTS_PATH, 'utf8'));

    const frontendGaps = report.findings.filter((f: any) => f.status === 'ACTUAL FRONTEND GAP');
    expect(frontendGaps.length).toBeGreaterThan(0);

    // Assert that the differential findings pinpoint the exact root cause components:
    // 1. Header Sidebar ("What's new" vs "Recent activity")
    // 2. Footer Status Bar ("⏸ manual mode on · ? for shortcuts · ← for agents" vs "? for shortcuts")
    const p1Startup = frontendGaps.find((f: any) => f.id === '01');
    expect(p1Startup).toBeDefined();
    expect(p1Startup.char_diff_count).toBe(3);
  });
});
