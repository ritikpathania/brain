const scriptIdx = process.argv.findIndex(arg => arg.endsWith('main.tsx') || arg.endsWith('main.js') || arg.endsWith('main.ts'));
if (scriptIdx > 1) {
  process.argv = [process.argv[0], process.argv[scriptIdx], ...process.argv.slice(scriptIdx + 1)];
}

import { plugin } from 'bun';
import * as os from 'os';
import * as path from 'path';
import * as fs from 'fs';

import React from 'react';

process.env.DISABLE_AUTOUPDATER = process.env.DISABLE_AUTOUPDATER || '1';
process.env.DISABLE_INSTALLATION_CHECKS = process.env.DISABLE_INSTALLATION_CHECKS || '1';
process.env.NODE_ENV = 'production';
process.env.USE_BUILTIN_RIPGREP = '0';
process.env.CLAUDE_CODE_NO_FLICKER = process.env.CLAUDE_CODE_NO_FLICKER || '1';

const cargoBin = path.join(os.homedir(), '.cargo', 'bin');
if (!process.env.PATH?.includes(cargoBin)) {
  process.env.PATH = `${cargoBin}:${process.env.PATH || ''}`;
}

if (process.env.BRAIN_CALLER_CWD && fs.existsSync(process.env.BRAIN_CALLER_CWD)) {
  try {
    process.chdir(process.env.BRAIN_CALLER_CWD);
  } catch {}
}

(globalThis as any).__BRAIN_PRELOAD_LOADED = true;

process.on('uncaughtException', (err) => {
  fs.writeFileSync('/tmp/brain_crash.log', 'UNCAUGHT: ' + String(err?.stack || err) + '\n', { flag: 'a' });
  process.stderr.write('Uncaught error in Brain Shell: ' + String(err?.stack || err) + '\n');
});
process.on('unhandledRejection', (err) => {
  fs.writeFileSync('/tmp/brain_crash.log', 'UNHANDLED_REJECTION: ' + String((err as any)?.stack || err) + '\n', { flag: 'a' });
});
// Polyfill React.useEffectEvent for custom reconcilers
const ReactSharedInternals = (React as any).__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE || (React as any).__SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED;
if (ReactSharedInternals) {
  let currentDispatcher = ReactSharedInternals.H;
  Object.defineProperty(ReactSharedInternals, 'H', {
    configurable: true,
    enumerable: true,
    get() {
      if (currentDispatcher && !currentDispatcher.useEffectEvent) {
        currentDispatcher.useEffectEvent = function(callback: any) {
          const ref = currentDispatcher.useRef(callback);
          ref.current = callback;
          return currentDispatcher.useCallback((...args: any[]) => ref.current(...args), []);
        };
      }
      return currentDispatcher;
    },
    set(v) {
      if (v && !v.useEffectEvent) {
        v.useEffectEvent = function(callback: any) {
          const ref = v.useRef(callback);
          ref.current = callback;
          return v.useCallback((...args: any[]) => ref.current(...args), []);
        };
      }
      currentDispatcher = v;
    }
  });
}

// Define MACRO globals if not already defined
(globalThis as any).MACRO = {
  VERSION: process.env.CLAUDE_VERSION || '2.1.235',
  PACKAGE_URL: '@anthropic-ai/claude-code',
  ISSUES_EXPLAINER: 'https://github.com/anthropics/claude-code/issues',
  README_URL: 'https://docs.anthropic.com/en/docs/claude-code',
  VERSION_CHANGELOG: 'Claude Code v2.1.235 release.',
  BUILD_TIME: '2026-08-16T12:00:00.000Z',
  ...((globalThis as any).MACRO || {}),
};

const BRAIN_SHELL_DIR = path.resolve(import.meta.dir, '..');
const VENDOR_CLAUDE_DIR = path.join(BRAIN_SHELL_DIR, 'vendor', 'claude');

function resolveTsPath(baseDir: string, subpath: string): string {
  const clean = subpath.replace(/\.jsx?$/, '');
  const candidates = [
    path.join(baseDir, subpath),
    path.join(baseDir, `${clean}.tsx`),
    path.join(baseDir, `${clean}.ts`),
    path.join(baseDir, `${clean}.jsx`),
    path.join(baseDir, `${clean}.js`),
    path.join(baseDir, `${clean}`, 'index.tsx'),
    path.join(baseDir, `${clean}`, 'index.ts'),
    path.join(baseDir, `${clean}`, 'index.js'),
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) return c;
  }
  return path.join(baseDir, subpath);
}

// Plugin to resolve shims and enforce hermetic vendor resolution for packages/brain-shell
plugin({
  name: 'claude-vendor-shims',
  setup(build) {
    build.onResolve({ filter: /^color-diff-napi$/ }, () => {
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'colorDiff.ts') };
    });

    build.onResolve({ filter: /^@alcalzone\/ansi-tokenize/ }, () => {
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'ansiTokenize.ts') };
    });

    build.onResolve({ filter: /ansi-tokenize[/\\]build[/\\](index|diff)\.js$/ }, () => {
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'ansiTokenize.ts') };
    });

    build.onResolve({ filter: /^\.\/types(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('filePersistence')) {
        return { path: 'filePersistence-types' };
      }
      return undefined;
    });

    build.onResolve({ filter: /feedConfigs(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('LogoV2')) {
        // Shim deleted; resolve to the vendor original it wrapped.
        return { path: path.join(VENDOR_CLAUDE_DIR, 'components', 'LogoV2', 'feedConfigs.tsx') };
      }
      return undefined;
    });



    build.onResolve({ filter: /\/Clawd(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('Clawd.tsx')) {
        return undefined;
      }
      // Shim deleted; resolve to the vendor original it wrapped.
      return { path: path.join(VENDOR_CLAUDE_DIR, 'components', 'LogoV2', 'Clawd.tsx') };
    });

    build.onResolve({ filter: /Opus1mMergeNotice(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('Opus1mMergeNotice.tsx')) {
        return undefined;
      }
      // Shim deleted; resolve to the vendor original it wrapped.
      return { path: path.join(VENDOR_CLAUDE_DIR, 'components', 'LogoV2', 'Opus1mMergeNotice.tsx') };
    });


    build.onResolve({ filter: /(?:^|[/\\])deps(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('brainQueryDeps')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'brainQueryDeps.ts') };
    });

    build.onResolve({ filter: /(?:^|[/\\])query(\.js)?$/ }, (args) => {
      if (args.importer && (args.importer.includes('brainQuery') || args.importer.includes('QueryEngine') || args.path.includes('query/') || args.importer.includes('/contracts/'))) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'brainQuery.ts') };
    });

    build.onResolve({ filter: /services\/api\/claude(\.js)?$/ }, (args) => {
      // The old shim layer is gone; resolve to the vendor original it wrapped.
      if (args.importer && args.importer.includes('vendor/claude/services/api/')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'vendor', 'claude', 'services', 'api', 'claude.ts') };
    });

    build.onResolve({ filter: /commands\/agents\/index(\.js)?$/ }, () => {
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'agentsCommand.ts') };
    });

    build.onResolve({ filter: /resume(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('commands/resume')) {
        return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'resumeCommand.tsx') };
      }
      return undefined;
    });

    build.onResolve({ filter: /LogSelector(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('shims/LogSelector.tsx')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'LogSelector.tsx') };
    });

    build.onResolve({ filter: /UserLocalCommandOutputMessage(\.js)?$/ }, () => {
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'UserLocalCommandOutputMessage.tsx') };
    });

    build.onResolve({ filter: /commands\/memory\/index(\.js)?$/ }, () => {
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'memoryCommand.ts') };
    });

    build.onResolve({ filter: /commands\/memory\/memory(\.js)?$/ }, () => {
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'memory.tsx') };
    });

    build.onResolve({ filter: /commands\/permissions\/index(\.js)?$/ }, () => {
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'permissionsCommand.ts') };
    });

    build.onResolve({ filter: /skills\/bundled\/index(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('bundledSkillsIndex.ts')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'bundledSkillsIndex.ts') };
    });

    build.onResolve({ filter: /\/PermissionMode(\.js)?$/ }, () => {
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'PermissionMode.ts') };
    });

    build.onResolve({ filter: /(?:utils\/|\.\/|^)ShellCommand(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('shims/ShellCommand.ts')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'ShellCommand.ts') };
    });





    build.onResolve({ filter: /useTextInput(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('shims/useTextInput.ts')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'useTextInput.ts') };
    });

    build.onResolve({ filter: /useVimInput(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('shims/useVimInput.ts')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'useVimInput.ts') };
    });

    build.onResolve({ filter: /Notifications(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('PromptInput')) {
        // Shim deleted; resolve to the vendor original it wrapped.
        return { path: path.join(VENDOR_CLAUDE_DIR, 'components', 'PromptInput', 'Notifications.tsx') };
      }
      return undefined;
    });


    build.onResolve({ filter: /StatusNotices(\.js)?$/ }, () => {
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'StatusNotices.tsx') };
    });

    build.onResolve({ filter: /commandSuggestions(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('shims/commandSuggestions.ts')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'commandSuggestions.ts') };
    });


    build.onResolve({ filter: /PromptInputFooterSuggestions(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('PromptInputFooterSuggestions.')) {
        return undefined;
      }
      // Shim deleted; resolve to the vendor original it wrapped.
      return { path: path.join(VENDOR_CLAUDE_DIR, 'components', 'PromptInput', 'PromptInputFooterSuggestions.tsx') };
    });

    build.onResolve({ filter: /PromptInputFooterLeftSide(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('PromptInputFooterLeftSide.')) {
        return undefined;
      }
      // Shim deleted; resolve to the vendor original it wrapped.
      return { path: path.join(VENDOR_CLAUDE_DIR, 'components', 'PromptInput', 'PromptInputFooterLeftSide.tsx') };
    });

    build.onResolve({ filter: /PromptInputFooter(\.js)?$/ }, (args) => {
      if (args.importer && (args.importer.includes('PromptInput/PromptInputFooter.') || args.importer.includes('PromptInputFooterSuggestions') || args.importer.includes('PromptInputFooterLeftSide'))) {
        return undefined;
      }
      // Shim deleted; resolve to the vendor original it wrapped.
      return { path: path.join(VENDOR_CLAUDE_DIR, 'components', 'PromptInput', 'PromptInputFooter.tsx') };
    });

    build.onResolve({ filter: /permissionOptions(\.js)?$/ }, (args) => {
      // Shim deleted; resolve to the vendor original it wrapped.
      if (args.importer && args.importer.includes('vendor/claude/components/permissions/')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'vendor', 'claude', 'components', 'permissions', 'FilePermissionDialog', 'permissionOptions.tsx') };
    });

    build.onResolve({ filter: /shellPermissionHelpers(\.js)?$/ }, (args) => {
      // Shim deleted; resolve to the vendor original it wrapped.
      return { path: path.join(BRAIN_SHELL_DIR, 'vendor', 'claude', 'components', 'permissions', 'shellPermissionHelpers.tsx') };
    });

    build.onResolve({ filter: /PermissionRuleList(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('rules/PermissionRuleList.')) {
        return undefined;
      }
      // Shim deleted; resolve to the vendor original it wrapped.
      return { path: path.join(VENDOR_CLAUDE_DIR, 'components', 'permissions', 'rules', 'PermissionRuleList.tsx') };
    });

    build.onResolve({ filter: /ListItem(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('shims/ListItem.tsx')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'ListItem.tsx') };
    });



    build.onResolve({ filter: /wrapAnsi(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('shims/wrapAnsi.ts')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'wrapAnsi.ts') };
    });

    build.onResolve({ filter: /(CustomSelect\/select|\.\/select)(\.js)?$/ }, (args) => {
      // Shim deleted; resolve to the vendor original it wrapped.
      if (args.importer && args.importer.includes('vendor/claude/components/CustomSelect/select.')) {
        return undefined;
      }
      if (!args.path.includes('CustomSelect') && !args.importer?.includes('CustomSelect')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'vendor', 'claude', 'components', 'CustomSelect', 'select.tsx') };
    });

    build.onResolve({ filter: /design-system\/Tabs(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('design-system/Tabs.')) {
        return undefined;
      }
      // Shim deleted; resolve to the vendor original it wrapped.
      return { path: path.join(VENDOR_CLAUDE_DIR, 'components', 'design-system', 'Tabs.tsx') };
    });

    build.onResolve({ filter: /ThemePicker(\.js)?$/ }, (args) => {
      if (args.importer && (args.importer.includes('commands/theme') || args.importer.includes('Onboarding') || args.importer.includes('Config'))) {
        return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'ThemePicker.tsx') };
      }
      return undefined;
    });

    build.onResolve({ filter: /ThemeProvider(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('shims/ThemeProvider.tsx')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'ThemeProvider.tsx') };
    });

    build.onResolve({ filter: /\/LogoV2(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('LogoV2/LogoV2.')) {
        return undefined;
      }
      // Shim deleted; resolve to the vendor original it wrapped.
      return { path: path.join(VENDOR_CLAUDE_DIR, 'components', 'LogoV2', 'LogoV2.tsx') };
    });

    build.onResolve({ filter: /UserCommandMessage(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('shims/UserCommandMessage.tsx')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'UserCommandMessage.tsx') };
    });

    build.onResolve({ filter: /HighlightedThinkingText(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('shims/HighlightedThinkingText.tsx')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'HighlightedThinkingText.tsx') };
    });

    build.onResolve({ filter: /log-update(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('shims/logUpdate.ts')) {
        return undefined;
      }
      // Stock ink bundles its own renderer at build/log-update.js and imports it
      // relatively; only the vendored ink fork gets our cell-renderer shim.
      if (args.importer && args.importer.includes('/node_modules/')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'logUpdate.ts') };
    });

    build.onResolve({ filter: /(?:bundled\/verify|^\.\/verify)(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('bundled')) {
        // Shim deleted; resolve to the vendor original it wrapped.
        return { path: path.join(VENDOR_CLAUDE_DIR, 'skills', 'bundled', 'verify.ts') };
      }
      return undefined;
    });

    build.onResolve({ filter: /^src\// }, (args) => {
      const subpath = args.path.replace(/^src\//, '');
      const res = resolveTsPath(VENDOR_CLAUDE_DIR, subpath);
      return { path: res };
    });

    build.onResolve({ filter: /screens\/REPL(\.js)?$/ }, (args) => {
      if (args.importer && args.importer.includes('shims/REPL.tsx')) {
        return undefined;
      }
      return { path: path.join(BRAIN_SHELL_DIR, 'src', 'shims', 'REPL.tsx') };
    });

    build.onResolve({ filter: /^\.{1,2}\/.*\.jsx?$/ }, (args) => {
      if (args.importer && args.importer.includes('shims/REPL.tsx')) {
        const shimPath = resolveTsPath(path.join(BRAIN_SHELL_DIR, 'src', 'shims'), args.path);
        if (fs.existsSync(shimPath)) {
          return { path: shimPath };
        }
        const dir = path.join(VENDOR_CLAUDE_DIR, 'screens');
        const resolved = resolveTsPath(dir, args.path);
        return { path: resolved };
      }
      if (args.importer && (args.importer.includes('/vendor/claude/') || args.importer.includes('/shims/') || args.importer.includes('src/shims'))) {
        const dir = path.dirname(args.importer);
        const resolved = resolveTsPath(dir, args.path);
        return { path: resolved };
      }
      return undefined;
    });

    build.onResolve({ filter: /\.md$/ }, (args) => {
      const resolved = args.importer ? path.resolve(path.dirname(args.importer), args.path) : args.path;
      return { path: resolved };
    });

    build.onLoad({ filter: /\.md$/ }, (args) => {
      try {
        const text = fs.readFileSync(args.path, 'utf8');
        return { contents: `export default ${JSON.stringify(text)};`, loader: 'js' };
      } catch {
        return { contents: `export default '';`, loader: 'js' };
      }
    });








    build.module('bun:bundle', () => ({
      contents: `
        export function feature(flag) {
          if (flag === 'AUTO_THEME') return true;
          if (flag === 'TRANSCRIPT_CLASSIFIER') return true;
          if (flag === 'TERMINAL_PANEL') return true;
          return false;
        }
      `,
      loader: 'js',
    }));

    build.module('@ant/claude-for-chrome-mcp', () => ({
      contents: `
        export const BROWSER_TOOLS = [];
        export const CLAUDE_IN_CHROME_MCP_SERVER_NAME = 'claude-in-chrome';
        export default { BROWSER_TOOLS, CLAUDE_IN_CHROME_MCP_SERVER_NAME };
      `,
      loader: 'js',
    }));

    build.module('@ant/computer-use-mcp', () => ({
      contents: `
        export const DEFAULT_GRANT_FLAGS = {};
        export const API_RESIZE_PARAMS = {};
        export const targetImageSize = () => null;
        export const buildComputerUseTools = () => [];
        export const bindSessionContext = () => {};
        export default { DEFAULT_GRANT_FLAGS, API_RESIZE_PARAMS, targetImageSize, buildComputerUseTools, bindSessionContext };
      `,
      loader: 'js',
    }));

    build.module('@ant/computer-use-mcp/sentinelApps', () => ({
      contents: `
        export const getSentinelCategory = () => null;
        export default { getSentinelCategory };
      `,
      loader: 'js',
    }));

    build.module('@ant/computer-use-mcp/types', () => ({
      contents: `
        export const DEFAULT_GRANT_FLAGS = {};
        export default { DEFAULT_GRANT_FLAGS };
      `,
      loader: 'js',
    }));

    build.module('filePersistence-types', () => ({
      contents: `
        export const DEFAULT_UPLOAD_CONCURRENCY = 5;
        export const FILE_COUNT_LIMIT = 100;
        export const OUTPUTS_SUBDIR = 'outputs';
      `,
      loader: 'js',
    }));
  },
});
