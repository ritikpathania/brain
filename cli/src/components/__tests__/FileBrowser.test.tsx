import React from 'react';
import { FileBrowser } from '../widgets/FileBrowser';
import { render as inkRender, cleanup } from 'ink-testing-library';
import { expect, test, describe, afterEach } from 'bun:test';
import { EventEmitter } from 'events';
import { FocusManager } from '../../services/FocusManager';
import fs from 'fs';
import { ThemeProvider } from '../design-system';

if (!(EventEmitter.prototype as any).ref) {
  (EventEmitter.prototype as any).ref = function() { return this; };
}
if (!(EventEmitter.prototype as any).unref) {
  (EventEmitter.prototype as any).unref = function() { return this; };
}

// Save original fs methods
const originalExistsSync = fs.existsSync;
const originalReaddirSync = fs.readdirSync;
const originalStatSync = fs.statSync;

afterEach(() => {
  cleanup();
  FocusManager.reset();
  fs.existsSync = originalExistsSync;
  fs.readdirSync = originalReaddirSync;
  fs.statSync = originalStatSync;
});

const render = (tree: React.ReactNode) => {
  return inkRender(tree);
};

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe('FileBrowser Widget', () => {
  test('renders empty directory message when no files exist', async () => {
    fs.existsSync = () => true;
    fs.readdirSync = () => [];

    const { lastFrame } = render(
      <ThemeProvider>
        <FileBrowser isFocused={true} visible={true} />
      </ThemeProvider>
    );

    await sleep(50);
    const frame = lastFrame();
    expect(frame).toContain('Empty directory.');
  });

  test('implements sliding window pagination when files exceed 8', async () => {
    fs.existsSync = () => true;
    fs.readdirSync = () => [
      'file01.txt', 'file02.txt', 'file03.txt', 'file04.txt',
      'file05.txt', 'file06.txt', 'file07.txt', 'file08.txt',
      'file09.txt', 'file10.txt', 'file11.txt', 'file12.txt'
    ];
    fs.statSync = () => ({
      isDirectory: () => false,
    } as any);

    const { lastFrame } = render(
      <ThemeProvider>
        <FileBrowser isFocused={true} visible={true} />
      </ThemeProvider>
    );

    await sleep(50);
    let frame = lastFrame();

    // 1. Initially, should show first 8 items (file01 to file08) and bottom indicator
    expect(frame).toContain('file01.txt');
    expect(frame).toContain('file08.txt');
    expect(frame).not.toContain('file09.txt');
    expect(frame).toContain('▼ 4 more item(s)...');
    expect(frame).not.toContain('▲');

    // 2. Focus the FileBrowser widget and navigate down
    const active = FocusManager.getActiveWidget();
    expect(active).not.toBeNull();
    expect(active?.id).toBe('file-browser');

    // Press Down 6 times to select file07 (centered window)
    for (let i = 0; i < 6; i++) {
      active?.handleInput('', { downArrow: true });
    }
    await sleep(50);
    frame = lastFrame();
    // Centered window should show file03 to file10 with indicators on both sides
    expect(frame).not.toContain('file01.txt');
    expect(frame).toContain('file03.txt');
    expect(frame).toContain('file10.txt');
    expect(frame).toContain('▲ 2 more item(s)...');
    expect(frame).toContain('▼ 2 more item(s)...');

    // Press Down 2 more times (total 8 presses, index 8 -> file09.txt)
    active?.handleInput('', { downArrow: true });
    active?.handleInput('', { downArrow: true });
    await sleep(50);
    frame = lastFrame();

    // Now window has slid. Should show file05 to file12, and top indicators
    expect(frame).not.toContain('file01.txt');
    expect(frame).not.toContain('file03.txt');
    expect(frame).toContain('file05.txt');
    expect(frame).toContain('file12.txt');
    expect(frame).toContain('▲ 4 more item(s)...');
    expect(frame).not.toContain('▼'); // Since 12 is the last file, no more below
  });
});
