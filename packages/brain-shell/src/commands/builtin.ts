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
    argumentHint: '<add <label> [--content <text>] | delete <id>>',
    run: (ctx) => {
      const sub = ctx.args[0]?.toLowerCase();
      if (sub === 'delete') {
        const targetId = ctx.args[1];
        if (!targetId) {
          return { type: 'text', value: 'Usage: /memory delete <node_id>' };
        }
        return { type: 'action', action: 'memory-delete', args: [targetId] };
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
      return { type: 'overlay', overlay: 'memory' };
    },
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
];

for (const cmd of BUILTINS) registerCommand(cmd);
