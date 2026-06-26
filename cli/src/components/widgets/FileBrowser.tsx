import React, { useState, useEffect } from 'react';
import fs from 'fs';
import path from 'path';
import { ThemedBox, ThemedText } from '../design-system';
import { WidgetContainer, WidgetHeader, WidgetBody, WidgetFooter } from './base/Widget';
import { InteractiveWidget, WidgetState } from './base/InteractiveWidget';
import { FocusManager } from '../../services/FocusManager';
import { EventBus } from '../../services/EventBus';

interface FileEntry {
  name: string;
  isDirectory: boolean;
  absolutePath: string;
}

export class FileBrowserWidget implements InteractiveWidget {
  public id = 'file-browser';
  public title = 'File Browser';
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

  public onMount() {
    this.setFocused(false);
  }
}

interface FileBrowserProps {
  isFocused?: boolean;
  visible?: boolean;
}

export const FileBrowser: React.FC<FileBrowserProps> = ({ isFocused = false, visible = true }) => {
  const homeDir = process.env.HOME || '/tmp';
  const rootDir = path.join(homeDir, '.brain');

  const [currentDir, setCurrentDir] = useState(rootDir);
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [widgetState, setWidgetState] = useState<WidgetState>('idle');
  const [errorMsg, setErrorMsg] = useState('');
  const [focused, setFocusedState] = useState(isFocused);

  // Load files in the directory
  const loadDirectory = (dir: string) => {
    setWidgetState('loading');
    try {
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
      }

      const files = fs.readdirSync(dir);
      const mapped = files
        .filter(f => !f.startsWith('.')) // omit hidden files
        .map(file => {
          const absolutePath = path.join(dir, file);
          let isDirectory = false;
          try {
            isDirectory = fs.statSync(absolutePath).isDirectory();
          } catch (e) {}
          return { name: file, isDirectory, absolutePath };
        })
        // directories first, then alphabetically
        .sort((a, b) => {
          if (a.isDirectory && !b.isDirectory) return -1;
          if (!a.isDirectory && b.isDirectory) return 1;
          return a.name.localeCompare(b.name);
        });

      setEntries(mapped);
      setSelectedIndex(0);
      setWidgetState('idle');
    } catch (e: any) {
      setErrorMsg(e.message);
      setWidgetState('error');
    }
  };

  useEffect(() => {
    loadDirectory(currentDir);
  }, [currentDir]);

  // Handle inputs delegated to this widget
  const handleWidgetInput = (input: string, key: any): boolean => {
    if (key.upArrow) {
      setSelectedIndex((prev) => Math.max(0, prev - 1));
      return true;
    }
    if (key.downArrow) {
      setSelectedIndex((prev) => Math.min(entries.length - 1, prev + 1));
      return true;
    }
    if (key.return) {
      if (entries.length === 0) return true;
      const selected = entries[selectedIndex];
      if (selected.isDirectory) {
        setCurrentDir(selected.absolutePath);
      } else {
        // Read file contents or display info via toast
        try {
          const content = fs.readFileSync(selected.absolutePath, 'utf8');
          const preview = content.slice(0, 150).replace(/\r?\n/g, ' ') + (content.length > 150 ? '...' : '');
          EventBus.publish({
            type: 'ToastAdded',
            message: `File: ${selected.name} | Contents: "${preview}"`,
          });
        } catch (e: any) {
          EventBus.publish({
            type: 'ToastAdded',
            message: `Error reading file: ${e.message}`,
          });
        }
      }
      return true;
    }
    if (key.escape || key.leftArrow) {
      // Go up a directory, but do not escape rootDir
      if (currentDir !== rootDir && currentDir.startsWith(rootDir)) {
        setCurrentDir(path.dirname(currentDir));
      } else {
        EventBus.publish({
          type: 'ToastAdded',
          message: 'Already at config root directory.',
        });
      }
      return true;
    }
    return false;
  };

  const handlerRef = React.useRef(handleWidgetInput);
  handlerRef.current = handleWidgetInput;

  useEffect(() => {
    if (!visible) return;
    const widget = new FileBrowserWidget(
      (val) => setFocusedState(val),
      (input, key) => handlerRef.current(input, key)
    );
    FocusManager.register(widget);
    return () => {
      FocusManager.unregister(widget.id);
    };
  }, [visible]);

  const shortcuts = [
    { key: '↑/↓', description: 'Navigate entries' },
    { key: 'Enter', description: 'Enter directory / read file' },
    { key: 'Esc/←', description: 'Go up directory' },
  ];

  const relPath = path.relative(rootDir, currentDir);
  const footerStatus = `Dir: .brain/${relPath}`;

  if (!visible) return <ThemedBox />;

  return (
    <WidgetContainer isFocused={focused}>
      <WidgetHeader
        title={`File Browser (.brain/${relPath})`}
        isFocused={focused}
        state={widgetState}
        errorMessage={errorMsg}
      />
      <WidgetBody state={widgetState} errorMessage={errorMsg}>
        {entries.length === 0 && widgetState === 'idle' ? (
          <ThemedBox padding={1}>
            <ThemedText color="inactive" italic>Empty directory.</ThemedText>
          </ThemedBox>
        ) : (
          <ThemedBox flexDirection="column">
            {entries.map((entry, idx) => {
              const isSelected = idx === selectedIndex;
              let color = 'text';
              if (isSelected) {
                color = 'claude';
              } else if (entry.isDirectory) {
                color = 'professionalBlue';
              }

              const marker = isSelected ? '▶ ' : '  ';
              const nameSuffix = entry.isDirectory ? '/' : '';

              return (
                <ThemedBox key={entry.name} flexDirection="row">
                  <ThemedText color={isSelected ? 'claude' : 'inactive'} bold>
                    {marker}
                  </ThemedText>
                  <ThemedText color={color} bold={entry.isDirectory || isSelected}>
                    {entry.name}
                    {nameSuffix}
                  </ThemedText>
                </ThemedBox>
              );
            })}
          </ThemedBox>
        )}
      </WidgetBody>
      <WidgetFooter shortcuts={shortcuts} statusText={footerStatus} />
    </WidgetContainer>
  );
};
