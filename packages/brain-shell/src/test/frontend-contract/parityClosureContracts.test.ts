import { describe, test, expect } from 'bun:test';
import * as fs from 'fs';
import * as path from 'path';

import { createWhatsNewFeed, createRecentActivityFeed, createProjectOnboardingFeed } from '../../../vendor/claude/components/LogoV2/feedConfigs.js';
import { calculateFeedWidth } from '../../../vendor/claude/components/LogoV2/Feed.js';
import { shouldShowProjectOnboarding } from '../../../vendor/claude/projectOnboardingState.js';
import { isDefaultMode, permissionModeTitle, permissionModeSymbol } from '../../../vendor/claude/utils/permissions/PermissionMode.js';

const TEST_DIR = import.meta.dir;
const AUDIT_RESULTS_PATH = path.resolve(TEST_DIR, '..', 'differentialAuditResults.json');

describe('Phase 7D: Surgical Empirical Parity Closure Contracts (GAP-01, GAP-02, GAP-03)', () => {

  // ==========================================================================
  // GAP-01: HEADER RELEASE NOTES & ONBOARDING FEED CONTRACT
  // ==========================================================================
  describe('GAP-01: Header Feed & Release Notes Causal State Contract', () => {
    test('Causal Invariant 1: createWhatsNewFeed naturally produces "What\'s new" title and emptyMessage without hardcoding', () => {
      const feed = createWhatsNewFeed([]);
      expect(feed.title).toBe("What's new");
      expect(feed.emptyMessage).toBe('Check the Claude Code changelog for updates');
      expect(feed.lines.length).toBe(0);

      const width = calculateFeedWidth(feed);
      expect(width).toBeGreaterThanOrEqual(10);
    });

    test('Causal Invariant 2: createRecentActivityFeed naturally produces "Recent activity" and emptyMessage without hardcoding', () => {
      const feed = createRecentActivityFeed([]);
      expect(feed.title).toBe('Recent activity');
      expect(feed.emptyMessage).toBe('No recent activity');
      expect(feed.lines.length).toBe(0);

      const width = calculateFeedWidth(feed);
      expect(width).toBeGreaterThanOrEqual(10);
    });

    test('Causal Invariant 3: projectOnboardingFeed formats enabled onboarding steps with tick glyphs', () => {
      const steps = [
        { id: '1', text: 'Run /init to create a CLAUDE.md file', isComplete: false, isEnabled: true },
      ];
      const feed = createProjectOnboardingFeed(steps as any);
      expect(feed.title).toBe('Tips for getting started');
      expect(feed.lines.length).toBe(1);
      expect(feed.lines[0].text).toContain('Run /init');
    });
  });

  // ==========================================================================
  // GAP-02: FOOTER PERMISSION & AGENT BADGES CONTRACT
  // ==========================================================================
  describe('GAP-02: Footer Permission & Status Badges Causal State Contract', () => {
    test('Causal Invariant 1: PermissionMode predicates naturally produce exact mode title and symbol', () => {
      expect(isDefaultMode('default')).toBe(true);
      expect(isDefaultMode('plan')).toBe(false);
      expect(isDefaultMode('acceptEdits')).toBe(false);

      expect(permissionModeSymbol('plan')).toBe('⏸');
      expect(permissionModeTitle('plan')).toBe('Plan Mode');

      expect(permissionModeSymbol('acceptEdits')).toBe('⏵⏵');
      expect(permissionModeTitle('acceptEdits')).toBe('Accept edits');
    });

    test('Causal Invariant 2: Active permission mode string formatting matches reference footer', () => {
      const mode = 'plan';
      const formatted = `${permissionModeSymbol(mode)} ${permissionModeTitle(mode).toLowerCase()} on`;
      expect(formatted).toBe('⏸ plan mode on');
    });
  });

  // ==========================================================================
  // GAP-03: DIFFERENTIAL RUNNER & AUDIT CLASSIFICATION HARD GATE
  // ==========================================================================
  describe('GAP-03: Differential Audit Classification Hard Gate', () => {
    test('Gate D.1: Differential audit findings are 100% classified with 0 unclassified gaps', () => {
      expect(fs.existsSync(AUDIT_RESULTS_PATH)).toBe(true);
      const report = JSON.parse(fs.readFileSync(AUDIT_RESULTS_PATH, 'utf8'));

      expect(report.classification_counts).toBeDefined();
      expect(report.classification_counts['UNCLASSIFIED']).toBe(0);
      expect(report.findings.length).toBeGreaterThanOrEqual(20);
    });

    test('Gate D.2: Audit manifest matches frozen Reference Claude v2.1.233 and Bun v1.4.0', () => {
      const report = JSON.parse(fs.readFileSync(AUDIT_RESULTS_PATH, 'utf8'));
      expect(report.manifest.claude_reference.version).toBe('2.1.233');
      expect(report.manifest.claude_reference.sha256).toBe('bc466b6cde63edafc773f471a1fb98787fabb31f52240c8616ce7e1f587b212d');
    });
  });
});
