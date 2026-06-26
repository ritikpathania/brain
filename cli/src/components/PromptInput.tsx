import React, { useEffect, useState } from 'react';
import { useInput } from 'ink';
import { ThemedBox, ThemedText } from './design-system';
import { FocusManager } from '../services/FocusManager';
import { useCommandHistory } from '../hooks/useCommandHistory';
import { InteractiveWidget } from './widgets/base/InteractiveWidget';

interface PromptInputProps {
  onSubmit: (value: string) => void;
  placeholder?: string;
  prefix?: string;
}

export class PromptInputWidget implements InteractiveWidget {
  public id = 'prompt-input';
  public title = 'Command Prompt';
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

export const PromptInput: React.FC<PromptInputProps> = ({
  onSubmit,
  placeholder = 'Type a memory command (e.g. query, ingest) or exit...',
  prefix = 'Memory Engine> ',
}) => {
  const [focused, setFocusedState] = useState(false);
  const { value, setValue, appendHistory, handleArrowKeys } = useCommandHistory('');

  const callbackRef = React.useRef<(input: string, key: any) => boolean>(() => false);

  callbackRef.current = (input: string, key: any): boolean => {
    if (key.return) {
      const submitted = value.trim();
      if (submitted) {
        appendHistory(submitted);
        onSubmit(submitted);
        setValue('');
      }
      return true;
    }
    if (key.backspace || key.delete) {
      setValue((prev) => prev.slice(0, -1));
      return true;
    }
    if (key.upArrow || key.downArrow) {
      return handleArrowKeys(key);
    }
    if (input && !key.ctrl && !key.meta && !key.escape && input !== '\r' && input !== '\n' && input !== '\t' && !key.tab) {
      setValue((prev) => prev + input);
      return true;
    }
    return false;
  };

  useEffect(() => {
    const widget = new PromptInputWidget(
      (val) => setFocusedState(val),
      (input, key) => callbackRef.current(input, key)
    );
    FocusManager.register(widget);
    return () => {
      FocusManager.unregister(widget.id);
    };
  }, []);

  useInput((input, key) => {
    // Toggle active widget focus when pressing Tab
    if (input === '\t' || key.tab) {
      FocusManager.focusNext();
      return;
    }

    // Delegate input to active widget first
    const activeWidget = FocusManager.getActiveWidget();
    if (activeWidget && activeWidget.id !== 'prompt-input') {
      const consumed = activeWidget.handleInput(input, key);
      if (consumed) return;
    }

    // Direct input to prompt if active
    if (focused) {
      callbackRef.current(input, key);
    }
  });

  return (
    <ThemedBox flexDirection="row">
      <ThemedText color={focused ? 'claude' : 'inactive'} bold>
        {prefix}
      </ThemedText>
      {value.length === 0 ? (
        <ThemedText color="inactive" dimColor>
          {placeholder}
        </ThemedText>
      ) : (
        <ThemedText color="text">{value}</ThemedText>
      )}
      <ThemedText color="claude" bold>
        █
      </ThemedText>
    </ThemedBox>
  );
};
