import React from 'react';
import { render as inkRender, cleanup } from 'ink-testing-library';
import { expect, test, describe, afterEach } from 'bun:test';
import { EventEmitter } from 'events';
import { FocusManager } from '../../services/FocusManager';

if (!(EventEmitter.prototype as any).ref) {
  (EventEmitter.prototype as any).ref = function() { return this; };
}
if (!(EventEmitter.prototype as any).unref) {
  (EventEmitter.prototype as any).unref = function() { return this; };
}

afterEach(() => {
  cleanup();
  FocusManager.reset();
});

import { ThemeProvider } from '../design-system';
import {
  ThemeScenario,
  AlertScenario,
  SpinnerScenario,
  ResizeScenario,
  ToastScenario,
  HistoryScenario,
  VerificationApp,
} from '../../verify_tui';

// Wrapper render to patch mock stdin for Bun & Ink 4 compatibility
const render = (tree: React.ReactNode) => {
  const result = inkRender(tree);
  if (result.stdin) {
    const buffer: string[] = [];
    (result.stdin as any).read = () => {
      return buffer.shift() ?? null;
    };
    
    result.stdin.write = (data: any) => {
      if (typeof data === 'string') {
        buffer.push(data);
        result.stdin.emit('readable');
      }
      return true;
    };
  }
  return result;
};

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe('Verification Scenarios Correctness', () => {
  test('ThemeScenario matches snapshot', () => {
    const { lastFrame } = render(
      <ThemeProvider>
        <ThemeScenario />
      </ThemeProvider>
    );
    expect(lastFrame()).toMatchSnapshot();
  });

  test('AlertScenario matches snapshot', () => {
    const { lastFrame } = render(
      <ThemeProvider>
        <AlertScenario />
      </ThemeProvider>
    );
    expect(lastFrame()).toMatchSnapshot();
  });

  test('SpinnerScenario matches snapshot', () => {
    const { lastFrame } = render(
      <ThemeProvider>
        <SpinnerScenario />
      </ThemeProvider>
    );
    expect(lastFrame()).toMatchSnapshot();
  });

  test('ResizeScenario matches snapshot at different widths', () => {
    const { lastFrame: frameCompact } = render(
      <ThemeProvider>
        <ResizeScenario width={45} />
      </ThemeProvider>
    );
    expect(frameCompact()).toMatchSnapshot();

    const { lastFrame: frameWide } = render(
      <ThemeProvider>
        <ResizeScenario width={80} />
      </ThemeProvider>
    );
    expect(frameWide()).toMatchSnapshot();
  });

  test('ToastScenario matches snapshot when toggled', () => {
    const { lastFrame: frameHidden } = render(
      <ThemeProvider>
        <ToastScenario showToast={false} />
      </ThemeProvider>
    );
    expect(frameHidden()).toMatchSnapshot();

    const { lastFrame: frameVisible } = render(
      <ThemeProvider>
        <ToastScenario showToast={true} message="Test Toast Alert" />
      </ThemeProvider>
    );
    expect(frameVisible()).toMatchSnapshot();
  });

  test('HistoryScenario matches snapshot at different index positions', () => {
    const { lastFrame: frameFirst } = render(
      <ThemeProvider>
        <HistoryScenario index={0} />
      </ThemeProvider>
    );
    expect(frameFirst()).toMatchSnapshot();

    const { lastFrame: frameThird } = render(
      <ThemeProvider>
        <HistoryScenario index={2} />
      </ThemeProvider>
    );
    expect(frameThird()).toMatchSnapshot();
  });
});

describe('VerificationApp Golden Interaction Sequence', () => {
  test('initial render matches snapshot', () => {
    const { lastFrame } = render(
      <ThemeProvider>
        <VerificationApp />
      </ThemeProvider>
    );
    const frame = lastFrame();
    expect(frame).toContain('Manual Verification Harness');
    expect(frame).toContain('SYSTEM STATUS');
    expect(frame).toContain('CRITICAL FAULT');
    expect(frame).toMatchSnapshot();
  });

  test('cycling themes changes rendering output', async () => {
    const { stdin, lastFrame } = render(
      <ThemeProvider>
        <VerificationApp />
      </ThemeProvider>
    );
    await sleep(50);

    // Initial theme is DARK
    expect(lastFrame()).toContain('Current Theme: DARK');

    // Press 't' -> switch theme
    stdin.write('t');
    await sleep(50);
    expect(lastFrame()).toContain('Current Theme: LIGHT');
    expect(lastFrame()).toMatchSnapshot();

    // Press 't' again -> switch theme
    stdin.write('t');
    await sleep(50);
    expect(lastFrame()).toContain('Current Theme: DARK-DALTONIZED');
    expect(lastFrame()).toMatchSnapshot();
  });

  test('toggling toast displays/hides the Toast element', async () => {
    const { stdin, lastFrame } = render(
      <ThemeProvider>
        <VerificationApp />
      </ThemeProvider>
    );
    await sleep(50);

    // Default: toast is hidden
    expect(lastFrame()).not.toContain('Toast Notification Triggered!');

    // Press 's' -> toggle toast on
    stdin.write('s');
    await sleep(50);
    expect(lastFrame()).toContain('Toast Notification Triggered!');
    expect(lastFrame()).toMatchSnapshot();

    // Press 's' again -> toggle toast off
    stdin.write('s');
    await sleep(50);
    expect(lastFrame()).not.toContain('Toast Notification Triggered!');
  });

  test('navigating command history with arrow keys updates active selection', async () => {
    const { stdin, lastFrame } = render(
      <ThemeProvider>
        <VerificationApp />
      </ThemeProvider>
    );
    await sleep(50);

    // Index 0 selected initially: '➔  ingest first item'
    expect(lastFrame()).toContain('➔  ingest first item');

    // Press Down Arrow (\u001b[B)
    stdin.write('\u001b[B');
    await sleep(50);
    expect(lastFrame()).toContain('➔  query database');

    // Press Down Arrow again
    stdin.write('\u001b[B');
    await sleep(50);
    expect(lastFrame()).toContain('➔  ingest second item');
    expect(lastFrame()).toMatchSnapshot();

    // Press Up Arrow (\u001b[A)
    stdin.write('\u001b[A');
    await sleep(50);
    expect(lastFrame()).toContain('➔  query database');
  });

  test('resizing width with Left and Right Arrows updates width scenario', async () => {
    const { stdin, lastFrame } = render(
      <ThemeProvider>
        <VerificationApp />
      </ThemeProvider>
    );
    await sleep(50);

    // Initial width: 80 cols
    expect(lastFrame()).toContain('Resize Scenario (Width: 80 columns)');
    expect(lastFrame()).toContain('Flex Grow Content B');

    // Press Left Arrow (\u001b[D) -> Width decreases to 70
    stdin.write('\u001b[D');
    await sleep(50);
    expect(lastFrame()).toContain('Resize Scenario (Width: 70 columns)');

    // Press Left Arrow three more times -> Width decreases to 40
    stdin.write('\u001b[D');
    await sleep(50);
    stdin.write('\u001b[D');
    await sleep(50);
    stdin.write('\u001b[D');
    await sleep(50);
    expect(lastFrame()).toContain('Resize Scenario (Width: 40 columns)');
    // Flex Grow Content B is hidden at width < 60
    expect(lastFrame()).not.toContain('Flex Grow Content B');
    expect(lastFrame()).toMatchSnapshot();
  });
});
