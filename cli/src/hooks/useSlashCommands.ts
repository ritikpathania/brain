import { SlashCommandRegistry } from '../services/SlashCommandRegistry';

export const useSlashCommands = () => {
  const executeCommand = async (
    command: string,
    context: {
      setLogs: React.SetStateAction<any>;
      setTheme: (theme: any) => void;
      client: any;
    }
  ): Promise<boolean> => {
    const trimmed = command.trim();
    if (!trimmed.startsWith('/')) {
      return false; // Not a slash command
    }

    const parts = trimmed.slice(1).split(/\s+/);
    const cmdName = parts[0].toLowerCase();
    const args = parts.slice(1);

    const cmd = SlashCommandRegistry.getCommand(cmdName);
    const dispatcher = context.setLogs as any;

    if (cmd) {
      try {
        await cmd.execute(args, context);
      } catch (e: any) {
        dispatcher((prev: string[]) => [
          ...prev,
          `[Error executing slash command /${cmdName}]: ${e.message}`,
        ]);
      }
    } else {
      dispatcher((prev: string[]) => [
        ...prev,
        `Unknown slash command: /${cmdName}. Type /help for a list of valid commands.`,
      ]);
    }

    return true;
  };

  return { executeCommand };
};
