import React, { useState, useEffect } from 'react';
import { ThemedBox, ThemedText } from './design-system';
import { WidgetContainer, WidgetHeader, WidgetBody, WidgetFooter } from './widgets/base/Widget';
import { InteractiveWidget } from './widgets/base/InteractiveWidget';
import { FocusManager } from '../services/FocusManager';
import { EventBus } from '../services/EventBus';
import { SlashCommandRegistry } from '../services/SlashCommandRegistry';

interface PaletteItem {
  label: string;
  category: 'Commands' | 'Themes' | 'Models' | 'Actions';
  description: string;
  execute: () => void;
}

export class CommandPaletteWidget implements InteractiveWidget {
  public id = 'command-palette';
  public title = 'Command Palette';
  private setFocused: (val: boolean) => void;
  private onInputCallback: (input: string, key: any) => boolean;

  constructor(setFocused: (val: boolean) => void, onInputCallback: (input: string, key: any) => boolean) {
    this.setFocused = setFocused;
    this.onInputCallback = onInputCallback;
  }

  public handleInput(input: string, key: any): boolean {
    return this.onInputCallback(input, key);
  }

  public onFocus() {
    this.setFocused(true);
  }

  public onBlur() {
    this.setFocused(false);
  }
}

interface CommandPaletteProps {
  onClose: () => void;
  context: {
    setLogs: React.SetStateAction<any>;
    setTheme: (theme: any) => void;
    client: any;
  };
}

export const CommandPalette: React.FC<CommandPaletteProps> = ({ onClose, context }) => {
  const [filterText, setFilterText] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [focused, setFocusedState] = useState(false);

  // Define static list of launcher items
  const items: PaletteItem[] = [
    // Commands
    {
      label: '/help',
      category: 'Commands',
      description: 'Show list of all commands',
      execute: () => {
        SlashCommandRegistry.getCommand('help')?.execute([], context);
      },
    },
    {
      label: '/history',
      category: 'Commands',
      description: 'Show command logs history',
      execute: () => {
        SlashCommandRegistry.getCommand('history')?.execute([], context);
      },
    },
    {
      label: '/config',
      category: 'Commands',
      description: 'Print active path config details',
      execute: () => {
        SlashCommandRegistry.getCommand('config')?.execute([], context);
      },
    },
    // Themes
    {
      label: '/theme dark',
      category: 'Themes',
      description: 'Switch TUI to Dark mode',
      execute: () => {
        SlashCommandRegistry.getCommand('theme')?.execute(['dark'], context);
      },
    },
    {
      label: '/theme light',
      category: 'Themes',
      description: 'Switch TUI to Light mode',
      execute: () => {
        SlashCommandRegistry.getCommand('theme')?.execute(['light'], context);
      },
    },
    {
      label: '/theme dark-daltonized',
      category: 'Themes',
      description: 'Switch TUI to Daltonized Dark mode',
      execute: () => {
        SlashCommandRegistry.getCommand('theme')?.execute(['dark-daltonized'], context);
      },
    },
    {
      label: '/theme light-daltonized',
      category: 'Themes',
      description: 'Switch TUI to Daltonized Light mode',
      execute: () => {
        SlashCommandRegistry.getCommand('theme')?.execute(['light-daltonized'], context);
      },
    },
    {
      label: '/theme dark-ansi',
      category: 'Themes',
      description: 'Switch TUI to 16-color ANSI Dark mode',
      execute: () => {
        SlashCommandRegistry.getCommand('theme')?.execute(['dark-ansi'], context);
      },
    },
    {
      label: '/theme light-ansi',
      category: 'Themes',
      description: 'Switch TUI to 16-color ANSI Light mode',
      execute: () => {
        SlashCommandRegistry.getCommand('theme')?.execute(['light-ansi'], context);
      },
    },
    // Models
    {
      label: '/model llm python-default',
      category: 'Models',
      description: 'Configure standard python extractor model',
      execute: () => {
        SlashCommandRegistry.getCommand('model')?.execute(['llm', 'python-default'], context);
      },
    },
    {
      label: '/model embeddings sentence-transformers',
      category: 'Models',
      description: 'Configure active local embeddings provider',
      execute: () => {
        SlashCommandRegistry.getCommand('model')?.execute(['embeddings', 'sentence-transformers'], context);
      },
    },
    // Actions
    {
      label: '/clear',
      category: 'Actions',
      description: 'Clear display logs',
      execute: () => {
        SlashCommandRegistry.getCommand('clear')?.execute([], context);
      },
    },
    {
      label: '/exit',
      category: 'Actions',
      description: 'Quit REPL',
      execute: () => {
        SlashCommandRegistry.getCommand('exit')?.execute([], context);
      },
    },
  ];

  // Filter items based on user search text
  const filtered = items.filter(
    (item) =>
      item.label.toLowerCase().includes(filterText.toLowerCase()) ||
      item.category.toLowerCase().includes(filterText.toLowerCase()) ||
      item.description.toLowerCase().includes(filterText.toLowerCase())
  );

  // Clamp selection index if filtered list size changes
  useEffect(() => {
    if (selectedIndex >= filtered.length) {
      setSelectedIndex(Math.max(0, filtered.length - 1));
    }
  }, [filterText, filtered.length]);

  const callbackRef = React.useRef<(input: string, key: any) => boolean>(() => false);

  callbackRef.current = (input: string, key: any): boolean => {
    if (key.upArrow) {
      setSelectedIndex((prev) => Math.max(0, prev - 1));
      return true;
    }
    if (key.downArrow) {
      setSelectedIndex((prev) => Math.min(filtered.length - 1, prev + 1));
      return true;
    }
    if (key.return) {
      if (filtered.length > 0) {
        const item = filtered[selectedIndex];
        item.execute();
      }
      onClose();
      return true;
    }
    if (key.escape) {
      onClose();
      return true;
    }
    if (key.backspace || key.delete) {
      setFilterText((prev) => prev.slice(0, -1));
      return true;
    }
    if (input && !key.ctrl && !key.meta && !key.escape && input !== '\r' && input !== '\n' && input !== '\t') {
      setFilterText((prev) => prev + input);
      return true;
    }

    return false;
  };

  useEffect(() => {
    const widget = new CommandPaletteWidget(
      (val) => setFocusedState(val),
      (input, key) => callbackRef.current(input, key)
    );
    FocusManager.register(widget);
    // Explicitly target focus on mount
    FocusManager.focusWidget(widget.id);

    return () => {
      FocusManager.unregister(widget.id);
    };
  }, []);

  const shortcuts = [
    { key: '↑/↓', description: 'Navigate matches' },
    { key: 'Enter', description: 'Execute command' },
    { key: 'Esc', description: 'Close palette' },
  ];

  return (
    <ThemedBox
      flexDirection="column"
      borderStyle="round"
      borderColor="claude"
      padding={1}
      width="100%"
      backgroundColor="messageActionsBackground"
      marginY={1}
    >
      <WidgetHeader title="Command Launcher" isFocused={focused} />
      <WidgetBody>
        <ThemedBox flexDirection="row" borderStyle="classic" borderColor="subtle" padding={1} marginBottom={1}>
          <ThemedText color="inactive">Search command... </ThemedText>
          <ThemedText color="text" bold marginLeft={2}>
            {filterText}
          </ThemedText>
          <ThemedText color="claude" bold>█</ThemedText>
        </ThemedBox>

        {filtered.length === 0 ? (
          <ThemedBox padding={1}>
            <ThemedText color="inactive" italic>No matches found.</ThemedText>
          </ThemedBox>
        ) : (
          <ThemedBox flexDirection="column">
            {filtered.map((item, idx) => {
              const isSelected = idx === selectedIndex;
              let itemColor = 'text';
              if (isSelected) itemColor = 'claude';

              const marker = isSelected ? '▶ ' : '  ';

              return (
                <ThemedBox key={idx} flexDirection="row" justifyContent="space-between">
                  <ThemedBox flexDirection="row">
                    <ThemedText color={isSelected ? 'claude' : 'inactive'} bold>
                      {marker}
                    </ThemedText>
                    <ThemedText color={itemColor} bold={isSelected}>
                      {item.label}
                    </ThemedText>
                    <ThemedText color="inactive" marginLeft={2}>
                      - {item.description}
                    </ThemedText>
                  </ThemedBox>
                  <ThemedText color="professionalBlue" bold>
                    [{item.category.toUpperCase()}]
                  </ThemedText>
                </ThemedBox>
              );
            })}
          </ThemedBox>
        )}
      </WidgetBody>
      <WidgetFooter shortcuts={shortcuts} statusText={`Total matches: ${filtered.length}`} />
    </ThemedBox>
  );
};
