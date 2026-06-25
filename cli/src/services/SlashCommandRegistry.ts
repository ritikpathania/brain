import { EventBus } from './EventBus';
import { HistoryStore } from './HistoryStore';
import { ThemeType } from '../components/design-system';

export interface SlashCommand {
  name: string;
  aliases?: string[];
  description: string;
  usage?: string;
  execute(args: string[], context: {
    setLogs: React.SetStateAction<any>;
    setTheme: (theme: ThemeType) => void;
    client: any;
  }): Promise<void> | void;
}

class SlashCommandRegistryService {
  private commands: Map<string, SlashCommand> = new Map();

  public register(cmd: SlashCommand): void {
    this.commands.set(cmd.name.toLowerCase(), cmd);
    if (cmd.aliases) {
      for (const alias of cmd.aliases) {
        this.commands.set(alias.toLowerCase(), cmd);
      }
    }
  }

  public getCommand(name: string): SlashCommand | undefined {
    return this.commands.get(name.toLowerCase());
  }

  public getCommands(): SlashCommand[] {
    const list: SlashCommand[] = [];
    const seen = new Set<string>();
    for (const cmd of this.commands.values()) {
      if (!seen.has(cmd.name)) {
        seen.add(cmd.name);
        list.push(cmd);
      }
    }
    return list;
  }
}

export const SlashCommandRegistry = new SlashCommandRegistryService();

// Register default commands
SlashCommandRegistry.register({
  name: 'help',
  description: 'Displays help instructions for all commands.',
  usage: '/help',
  execute: (_args, { setLogs }) => {
    const list = SlashCommandRegistry.getCommands();
    let helpMsg = '=== brain CLI Slash Commands ===';
    for (const cmd of list) {
      helpMsg += `\n  ${cmd.name.padEnd(10)} - ${cmd.description} (Usage: ${cmd.usage || '/' + cmd.name})`;
    }
    helpMsg += '\n\nStandard actions:\n  query <text>   - Query the memory engine\n  ingest <text>  - Ingest a log entry to memory';
    
    // cast dispatcher to avoid type clashes
    const dispatcher = setLogs as any;
    dispatcher((prev: string[]) => [...prev, helpMsg]);
  }
});

SlashCommandRegistry.register({
  name: 'clear',
  description: 'Clears the log stream window.',
  usage: '/clear',
  execute: (_args, { setLogs }) => {
    const dispatcher = setLogs as any;
    dispatcher([]);
    EventBus.publish({ type: 'ToastAdded', message: 'Log screen cleared.' });
  }
});

SlashCommandRegistry.register({
  name: 'theme',
  description: 'Switches the TUI theme dynamically.',
  usage: '/theme [dark|light|dark-daltonized|light-daltonized|dark-ansi|light-ansi]',
  execute: (args, { setLogs, setTheme }) => {
    const validThemes = ['dark', 'light', 'dark-daltonized', 'light-daltonized', 'dark-ansi', 'light-ansi'];
    const dispatcher = setLogs as any;
    if (args.length === 0 || !validThemes.includes(args[0])) {
      dispatcher((prev: string[]) => [
        ...prev,
        `Invalid or missing theme. Usage: /theme [${validThemes.join('|')}]`,
      ]);
      return;
    }
    const selected = args[0] as ThemeType;
    setTheme(selected);
    EventBus.publish({ type: 'ThemeChanged', theme: selected });
    dispatcher((prev: string[]) => [...prev, `[System] Theme switched to: ${selected}`]);
  }
});

SlashCommandRegistry.register({
  name: 'history',
  description: 'Displays the log command history for this session.',
  usage: '/history',
  execute: (_args, { setLogs }) => {
    const history = HistoryStore.getHistory();
    const dispatcher = setLogs as any;
    if (history.length === 0) {
      dispatcher((prev: string[]) => [...prev, 'Command history is empty.']);
      return;
    }
    const msg = '=== Command History ===\n' + history.map((h, idx) => `  ${idx + 1}. ${h}`).join('\n');
    dispatcher((prev: string[]) => [...prev, msg]);
  }
});

SlashCommandRegistry.register({
  name: 'model',
  description: 'Shows active LLM & Embedding models or switches configuration.',
  usage: '/model [llm|embeddings] [model_name]',
  execute: (args, { setLogs }) => {
    const dispatcher = setLogs as any;
    if (args.length === 0) {
      dispatcher((prev: string[]) => [
        ...prev,
        '=== Active Models Configuration ===\nActive LLM: python-default\nActive Embeddings: sentence-transformers/all-MiniLM-L6-v2\n\nUsage to switch (mock): /model [llm|embeddings] [name]',
      ]);
    } else {
      const type = args[0].toLowerCase();
      const modelName = args.slice(1).join(' ');
      if (type !== 'llm' && type !== 'embeddings') {
        dispatcher((prev: string[]) => [...prev, 'Invalid type. Use /model llm or /model embeddings']);
        return;
      }
      dispatcher((prev: string[]) => [...prev, `[System] Model '${type}' switched to '${modelName}' (simulated)`]);
    }
  }
});

SlashCommandRegistry.register({
  name: 'config',
  description: 'Prints active configuration paths and settings.',
  usage: '/config',
  execute: (_args, { setLogs }) => {
    const dispatcher = setLogs as any;
    const msg = `=== Active Configuration ===\n` +
      `  • UDS Socket: ~/.brain/daemon.sock\n` +
      `  • SQLite LTM: ~/.brain/memory.db\n` +
      `  • DuckDB OLAP: ~/.brain/analytics.duckdb\n` +
      `  • Active LLM: python-default\n` +
      `  • Active Exporter: duckdb`;
    dispatcher((prev: string[]) => [...prev, msg]);
  }
});

SlashCommandRegistry.register({
  name: 'exit',
  aliases: ['quit'],
  description: 'Closes UDS connection and exits the CLI.',
  usage: '/exit',
  execute: (_args, { setLogs }) => {
    const dispatcher = setLogs as any;
    dispatcher((prev: string[]) => [...prev, '[System] Exiting client REPL...']);
    setTimeout(() => process.exit(0), 400);
  }
});
