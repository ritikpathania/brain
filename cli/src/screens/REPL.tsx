import React, { useEffect, useState, useRef } from 'react';
import { useInput } from 'ink';
import { SocketClient, ServerResponse } from '../services/SocketClient';
import { PromptInput } from '../components/PromptInput';
import { MainLayout } from '../components/MainLayout';
import { ChatView } from '../components/widgets/ChatView';
import { TimelineView } from '../components/widgets/TimelineView';
import { ToolActivityView } from '../components/widgets/ToolActivityView';
import { SessionBrowser } from '../components/widgets/SessionBrowser';
import { CommandPalette } from '../components/CommandPalette';
import { useSlashCommands } from '../hooks/useSlashCommands';
import { useTheme } from '../components/design-system/hooks';
import { EventBus } from '../services/EventBus';
import { FocusManager } from '../services/FocusManager';
import { ViewModelStore } from '../services/ViewModelStore';
import { EventController } from '../services/EventController';

interface REPLProps {
  client: SocketClient;
}

export const REPL: React.FC<REPLProps> = ({ client }) => {
  const { executeCommand } = useSlashCommands();
  const { themeType, setTheme } = useTheme();

  // Create store and controller references
  const storeRef = useRef<ViewModelStore | null>(null);
  if (!storeRef.current) {
    storeRef.current = new ViewModelStore();
  }
  const store = storeRef.current;

  const controllerRef = useRef<EventController | null>(null);
  if (!controllerRef.current) {
    controllerRef.current = new EventController(store, (warning) => {
      // Append warning logs as system messages
      store.dispatch({ type: 'execution_failed', error: warning });
    });
  }
  const controller = controllerRef.current;

  // Subscribe to ViewModelStore state updates
  const [snapshot, setSnapshot] = useState(store.getSnapshot());
  useEffect(() => {
    return store.subscribe((snap) => {
      setSnapshot(snap);
    });
  }, [store]);

  const [paletteOpen, setPaletteOpen] = useState(false);

  // Set prompt-input focused initially
  useEffect(() => {
    FocusManager.focusWidget('prompt-input');
  }, []);

  // Listen to UDS client messages
  useEffect(() => {
    client.connect();

    const unsubscribeLog = client.onLog((message) => {
      store.dispatch({ type: 'execution_failed', error: `[CLI Log] ${message}` });
    });

    const unsubscribeMsg = client.onMessage((msg: ServerResponse) => {
      if (msg && typeof msg === 'object' && 'type' in msg && msg.type.startsWith('stream_')) {
        controller.handleStreamEvent(msg as any);
      } else {
        // Handle legacy or static responses
        if (msg.status === 'ok' || msg.status === 'success') {
          store.dispatch({
            type: 'execution_completed',
            response: (msg as any).body || (msg as any).message || '',
          });
        } else {
          store.dispatch({
            type: 'execution_failed',
            error: (msg as any).body || (msg as any).message || 'Error',
          });
        }
      }
    });

    return () => {
      unsubscribeLog();
      unsubscribeMsg();
    };
  }, [client, controller, store]);

  // Handle keyboard shortcuts
  useInput((input, key) => {
    if (key.ctrl && input === 'c') {
      store.dispatch({ type: 'execution_cancelled' });
      return;
    }

    if (key.ctrl && input === 'p') {
      setPaletteOpen((prev) => !prev);
      return;
    }
  });

  const handleCommandSubmit = async (command: string) => {
    const trimmed = command.trim();
    if (!trimmed) return;

    EventBus.publish({ type: 'HistoryAdded', command: trimmed });

    if (trimmed.toLowerCase() === 'exit' || trimmed.toLowerCase() === 'quit') {
      setTimeout(() => process.exit(0), 400);
      return;
    }

    // Execute slash command
    const isSlash = await executeCommand(trimmed, {
      setLogs: () => {},
      setTheme,
      client,
    });
    if (isSlash) return;

    // Start running
    store.dispatch({
      type: 'execution_started',
      sessionId: 'default',
      prompt: trimmed,
    });

    // Send raw command verb
    let action = 'ingest';
    let payload = trimmed;
    const spaceIdx = trimmed.indexOf(' ');
    if (spaceIdx !== -1) {
      const commandVerb = trimmed.slice(0, spaceIdx).toLowerCase();
      if (commandVerb === 'query' || commandVerb === 'ingest') {
        action = commandVerb;
        payload = trimmed.slice(spaceIdx + 1).trim();
      }
    }
    client.send(action, payload);
  };

  return (
    <>
      <MainLayout state={snapshot.state}>
        {{
          chatView: <ChatView messages={snapshot.state.messages} />,
          timelineView: <TimelineView timeline={snapshot.state.timelineEntries} />,
          toolActivityView: <ToolActivityView tools={snapshot.state.toolInvocations} />,
          sessionBrowser: (isFocused) => (
            <SessionBrowser
              sessions={snapshot.state.sessionList}
              activeSessionId={snapshot.state.activeSessionId}
              onSwitchSession={(sid) => store.dispatch({ type: 'switch_session', sessionId: sid })}
              isFocused={isFocused}
            />
          ),
          promptInput: <PromptInput onSubmit={handleCommandSubmit} />,
        }}
      </MainLayout>

      {paletteOpen && (
        <CommandPalette
          onClose={() => setPaletteOpen(false)}
          context={{ setLogs: () => {}, setTheme, client }}
        />
      )}
    </>
  );
};
