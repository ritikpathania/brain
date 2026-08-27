/** Built-in slash commands (Inc 21). Later increments extend this list. */

import { registerCommand, getCommands, type Command } from './registry.js';
import { runPermissionsCommand } from '../state/permissionRules.js';

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
  { name: 'resume', description: 'Resume a previous session', run: () => ({ type: 'action', action: 'resume' }) },
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
    argumentHint: '[delete <id>]',
    run: (ctx) => {
      if (ctx.args[0] === 'delete') {
        const targetId = ctx.args[1];
        if (!targetId) {
          return { type: 'text', value: 'Usage: /memory delete <node_id>' };
        }
        return { type: 'action', action: 'memory-delete', args: [targetId] };
      }
      return { type: 'overlay', overlay: 'memory' };
    },
  },
];

for (const cmd of BUILTINS) registerCommand(cmd);
