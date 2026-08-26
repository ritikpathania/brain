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
  { name: 'quit', description: 'Exit Brain shell', aliases: ['q'], run: () => ({ type: 'action', action: 'quit' }) },
  { name: 'doctor', description: 'Run system diagnostics', run: () => ({ type: 'overlay', overlay: 'doctor' }) },
  { name: 'memory', description: 'Browse the knowledge graph', run: () => ({ type: 'overlay', overlay: 'memory' }) },
];

for (const cmd of BUILTINS) registerCommand(cmd);
