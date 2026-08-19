import { describe, test, expect } from 'bun:test';
import * as fs from 'fs';
import * as path from 'path';
import * as crypto from 'crypto';

const TEST_DIR = import.meta.dir;
const BRAIN_SHELL_DIR = path.resolve(TEST_DIR, '..', '..', '..');
const VENDOR_DIR = path.join(BRAIN_SHELL_DIR, 'vendor', 'claude');
const MANIFEST_PATH = path.join(BRAIN_SHELL_DIR, 'vendor', 'claude.manifest.sha256');

describe('Gate A Portable Vendor Integrity Contract', () => {
  test('vendor/claude tree matches committed vendor.manifest.sha256 exactly (1,925/1,925 files)', () => {
    expect(fs.existsSync(MANIFEST_PATH)).toBe(true);

    const manifestContent = fs.readFileSync(MANIFEST_PATH, 'utf8').trim();
    const manifestLines = manifestContent.split('\n').filter(Boolean);

    expect(manifestLines.length).toBe(1925);

    const manifestMap = new Map<string, string>();
    for (const line of manifestLines) {
      const [sha, relPath] = line.trim().split(/\s+/);
      manifestMap.set(relPath, sha);
    }

    // Verify every file on disk
    let verifiedCount = 0;
    const errors: string[] = [];

    for (const [relPath, expectedSha] of manifestMap.entries()) {
      const fullPath = path.join(VENDOR_DIR, relPath);
      if (!fs.existsSync(fullPath)) {
        errors.push(`Missing file: ${relPath}`);
        continue;
      }
      const fileBytes = fs.readFileSync(fullPath);
      const actualSha = crypto.createHash('sha256').update(fileBytes).digest('hex');
      if (actualSha !== expectedSha) {
        errors.push(`Checksum mismatch for ${relPath}: expected ${expectedSha}, got ${actualSha}`);
      } else {
        verifiedCount++;
      }
    }

    expect(errors).toEqual([]);
    expect(verifiedCount).toBe(1925);
  });
});
