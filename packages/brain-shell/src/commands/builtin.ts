/** Built-in slash commands (Inc 21). Later increments extend this list. */

import { registerCommand, getCommands, type Command } from './registry.js';
import { runPermissionsCommand } from '../state/permissionRules.js';
import { tokenizeArgs, parseMemoryAddArgs } from './memoryArgs.js';

const BUILTINS: Command[] = [
  {
    name: 'help',
    description: 'List available slash commands',
    run: () => ({
      type: 'text',
      value: [
        'Slash commands:',
        ...getCommands()
          .filter((c) => !c.hidden)
          .map((c) => `/${c.name} — ${c.description}`),
      ].join('\n'),
    }),
  },
  { name: 'clear', description: 'Clear the transcript', run: () => ({ type: 'action', action: 'clear' }) },
  {
    name: 'resume',
    description: 'Resume a previous session',
    argumentHint: '[all | session_id]',
    run: (ctx) => {
      if (ctx.args && ctx.args[0] === 'delete') {
        const targetId = ctx.args.slice(1).join(' ').trim();
        if (!targetId) {
          return { type: 'text', value: 'Usage: /resume delete <id_or_prefix>' };
        }
        return { type: 'action', action: 'session-delete', args: [targetId] };
      }
      if (ctx.args && (ctx.args[0] === 'all' || ctx.args[0] === '--all')) {
        return { type: 'action', action: 'resume-all', args: ctx.args.slice(1) };
      }
      return ctx.args && ctx.args.length > 0
        ? { type: 'action', action: 'resume', args: ctx.args }
        : { type: 'action', action: 'resume' };
    },
  },
  {
    name: 'session',
    description: 'Manage chat sessions',
    argumentHint: '<delete|pin|unpin|archive|restore> [id_or_prefix]',
    run: (ctx) => {
      const sub = ctx.args[0];
      const targetId = ctx.args.slice(1).join(' ').trim();

      if (sub === 'delete') {
        if (!targetId) {
          return { type: 'text', value: 'Usage: /session delete <id_or_prefix>' };
        }
        return { type: 'action', action: 'session-delete', args: [targetId] };
      }
      if (sub === 'pin') {
        return { type: 'action', action: 'session-pin', args: targetId ? [targetId] : [] };
      }
      if (sub === 'unpin') {
        return { type: 'action', action: 'session-unpin', args: targetId ? [targetId] : [] };
      }
      if (sub === 'archive') {
        return { type: 'action', action: 'session-archive', args: targetId ? [targetId] : [] };
      }
      if (sub === 'restore') {
        if (!targetId) {
          return { type: 'text', value: 'Usage: /session restore <id_or_prefix>' };
        }
        return { type: 'action', action: 'session-restore', args: [targetId] };
      }
      return { type: 'text', value: 'Usage: /session <delete|pin|unpin|archive|restore> [id_or_prefix]' };
    },
  },
  {
    name: 'pin',
    description: 'Pin the active or specified session',
    argumentHint: '[id_or_prefix]',
    run: (ctx) => ({ type: 'action', action: 'session-pin', args: ctx.args }),
  },
  {
    name: 'unpin',
    description: 'Unpin the active or specified session',
    argumentHint: '[id_or_prefix]',
    run: (ctx) => ({ type: 'action', action: 'session-unpin', args: ctx.args }),
  },
  {
    name: 'archive',
    description: 'Archive the active or specified session',
    argumentHint: '[id_or_prefix]',
    run: (ctx) => ({ type: 'action', action: 'session-archive', args: ctx.args }),
  },
  {
    name: 'restore',
    description: 'Restore an archived session',
    argumentHint: '<id_or_prefix>',
    run: (ctx) => {
      const target = ctx.args.join(' ').trim();
      if (!target) {
        return { type: 'text', value: 'Usage: /restore <id_or_prefix>' };
      }
      return { type: 'action', action: 'session-restore', args: [target] };
    },
  },
  { name: 'theme', description: 'Change the color theme', run: () => ({ type: 'action', action: 'theme' }) },
  {
    name: 'permissions',
    description: 'List or remove always-allow rules',
    run: (ctx) => ({ type: 'text', value: runPermissionsCommand(ctx.args) }),
  },
  {
    name: 'rename',
    description: 'Rename the active session',
    argumentHint: '<title>',
    run: (ctx) => {
      if (ctx.args.length === 0) {
        return { type: 'text', value: 'Usage: /rename <title>' };
      }
      return { type: 'action', action: 'rename', args: ctx.args };
    },
  },
  {
    name: 'fork',
    description: 'Fork the current session into a new session',
    argumentHint: '[title]',
    run: (ctx) => ({ type: 'action', action: 'fork', args: ctx.args }),
  },
  { name: 'quit', description: 'Exit Brain shell', aliases: ['q'], run: () => ({ type: 'action', action: 'quit' }) },
  { name: 'doctor', description: 'Run system diagnostics', run: () => ({ type: 'overlay', overlay: 'doctor' }) },
  {
    name: 'memory',
    description: 'Browse or manage the knowledge graph',
    argumentHint: '<add <label> [--content <text>] | delete <id> | link <src> <tgt> [rel] | unlink <src> <tgt> [rel] | consolidate>',
    run: (ctx) => {
      const sub = ctx.args[0]?.toLowerCase();
      if (sub === 'delete') {
        const targetId = ctx.args[1];
        if (!targetId) {
          return { type: 'text', value: 'Usage: /memory delete <node_id>' };
        }
        return { type: 'action', action: 'memory-delete', args: [targetId] };
      }
      if (sub === 'link') {
        const source = ctx.args[1];
        const target = ctx.args[2];
        const relation = ctx.args[3] || 'associated_with';
        if (!source || !target) {
          return { type: 'text', value: 'Usage: /memory link <source> <target> [relation]' };
        }
        return {
          type: 'action',
          action: 'memory-link',
          args: [source, target, relation],
        };
      }
      if (sub === 'unlink') {
        const source = ctx.args[1];
        const target = ctx.args[2];
        const relation = ctx.args[3] || 'associated_with';
        if (!source || !target) {
          return { type: 'text', value: 'Usage: /memory unlink <source> <target> [relation]' };
        }
        return {
          type: 'action',
          action: 'memory-unlink',
          args: [source, target, relation],
        };
      }
      if (sub === 'add') {
        const addTokens = tokenizeArgs(ctx.args.slice(1).join(' '));
        const res = parseMemoryAddArgs(addTokens.length > 0 ? addTokens : ctx.args.slice(1));
        if (!res.ok) {
          return { type: 'text', value: res.error };
        }
        return {
          type: 'action',
          action: 'memory-add',
          args: [res.data.label, res.data.content],
        };
      }
      if (sub === 'consolidate') {
        return { type: 'action', action: 'memory-consolidate' };
      }
      return { type: 'overlay', overlay: 'memory' };
    },
  },
  {
    name: 'link',
    description: 'Link two memory concepts with a directed relation edge',
    argumentHint: '<source> <target> [relation]',
    run: (ctx) => {
      const source = ctx.args[0];
      const target = ctx.args[1];
      const relation = ctx.args[2] || 'associated_with';
      if (!source || !target) {
        return { type: 'text', value: 'Usage: /link <source> <target> [relation]' };
      }
      return {
        type: 'action',
        action: 'memory-link',
        args: [source, target, relation],
      };
    },
  },
  {
    name: 'unlink',
    description: 'Remove a directed relation edge between two memory concepts',
    argumentHint: '<source> <target> [relation]',
    run: (ctx) => {
      const source = ctx.args[0];
      const target = ctx.args[1];
      const relation = ctx.args[2] || 'associated_with';
      if (!source || !target) {
        return { type: 'text', value: 'Usage: /unlink <source> <target> [relation]' };
      }
      return {
        type: 'action',
        action: 'memory-unlink',
        args: [source, target, relation],
      };
    },
  },
  {
    name: 'consolidate',
    description: 'Consolidate, merge duplicates, and prune stale edges in the knowledge graph',
    run: () => ({ type: 'action', action: 'memory-consolidate' }),
  },
  {
    name: 'context',
    description: 'Inspect active turn memory context provenance and telemetry',
    run: () => ({ type: 'overlay', overlay: 'context' }),
  },
  {
    name: 'search',
    description: 'Search historical conversation transcripts',
    argumentHint: '[query]',
    run: (ctx) =>
      ctx.args && ctx.args.length > 0
        ? { type: 'action', action: 'search', args: ctx.args }
        : { type: 'action', action: 'search' },
  },
  {
    name: 'model',
    description: 'Inspect or switch active LLM generation model',
    argumentHint: '[name_or_alias]',
    aliases: ['models'],
    run: (ctx) =>
      ctx.args && ctx.args.length > 0
        ? { type: 'action', action: 'model-switch', args: ctx.args }
        : { type: 'overlay', overlay: 'model' },
  },
  {
    name: 'thinking',
    description: 'Configure or inspect reasoning thinking token budget',
    argumentHint: '[tokens | off]',
    aliases: ['think'],
    run: (ctx) => {
      const arg = ctx.args[0]?.trim();
      if (!arg) {
        return { type: 'action', action: 'thinking-status' };
      }
      if (
        arg.toLowerCase() === 'off' ||
        arg.toLowerCase() === 'disable' ||
        arg.toLowerCase() === 'none'
      ) {
        return { type: 'action', action: 'thinking-set', args: ['off'] };
      }
      const budget = parseInt(arg, 10);
      if (isNaN(budget) || budget <= 0 || String(budget) !== arg) {
        return { type: 'text', value: 'Usage: /thinking [tokens | off]' };
      }
      return { type: 'action', action: 'thinking-set', args: [String(budget)] };
    },
  },
  {
    name: 'system',
    description: 'Inspect or configure custom session-level system prompt instructions',
    argumentHint: '[instructions | clear | off | reset]',
    aliases: ['instructions', 'sysprompt'],
    run: (ctx) => {
      if (!ctx.args || ctx.args.length === 0 || !ctx.args[0]?.trim()) {
        return { type: 'action', action: 'system-status' };
      }
      const text = ctx.args.join(' ');
      return { type: 'action', action: 'system-set', args: [text] };
    },
  },
];

for (const cmd of BUILTINS) registerCommand(cmd);
