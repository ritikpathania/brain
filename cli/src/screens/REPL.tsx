import React, { useEffect, useState } from 'react';
import { useInput, useStdout } from 'ink';
import { SocketClient, ServerResponse } from '../services/SocketClient';
import { PromptInput } from '../components/PromptInput';
import {
  ThemedBox,
  ThemedText,
  Divider,
  LogoV2,
  StatusLine,
} from '../components/design-system';
import { WidgetContainer, WidgetHeader, WidgetBody } from '../components/widgets/base/Widget';
import { Table } from '../components/widgets/Table';
import { FileBrowser } from '../components/widgets/FileBrowser';
import { MultiStepForm } from '../components/widgets/MultiStepForm';
import { CommandPalette } from '../components/CommandPalette';
import { MarkdownRenderer } from '../components/MarkdownRenderer';
import { parseMarkdown } from '../services/MarkdownParser';
import { useLogStore } from '../hooks/useLogStore';
import { useMetrics } from '../hooks/useMetrics';
import { useSlashCommands } from '../hooks/useSlashCommands';
import { useStreamingRenderer } from '../hooks/useStreamingRenderer';
import { useTheme } from '../components/design-system/hooks';
import { EventBus } from '../services/EventBus';
import { FocusManager } from '../services/FocusManager';

interface REPLProps {
  client: SocketClient;
}

export const REPL: React.FC<REPLProps> = ({ client }) => {
  const { logs, setLogs, addLog } = useLogStore();
  const { metrics, isConnected } = useMetrics(3000);
  const { executeCommand } = useSlashCommands();
  const {
    displayedText,
    isStreaming,
    progress,
    startStream,
    queueChunk,
    handleProgress,
    endStream,
    cancelStream,
  } = useStreamingRenderer(15, (warning) => addLog(warning));
  const { themeType, setTheme } = useTheme();
  const { stdout } = useStdout();

  const [isLoading, setIsLoading] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [activeWidgetId, setActiveWidgetId] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'file-browser' | 'config-wizard'>('file-browser');
  const [shouldFocusTab, setShouldFocusTab] = useState(false);

  useEffect(() => {
    if (shouldFocusTab) {
      const timer = setTimeout(() => {
        FocusManager.focusWidget(activeTab);
        setShouldFocusTab(false);
      }, 50);
      return () => clearTimeout(timer);
    }
  }, [activeTab, shouldFocusTab]);

  const [sessionStats, setSessionStats] = useState({
    inputTokens: 1420,
    outputTokens: 495,
    cost: 0.0031,
  });

  const isStreamingRef = React.useRef(isStreaming);
  const displayedTextRef = React.useRef(displayedText);

  React.useEffect(() => {
    isStreamingRef.current = isStreaming;
    displayedTextRef.current = displayedText;
  }, [isStreaming, displayedText]);

  // Check if terminal is wide enough for side-panel dashboard
  const isWide = (stdout?.columns ?? 80) >= 120;

  // Track active focus manager changes
  useEffect(() => {
    const unsubscribe = EventBus.subscribe((event) => {
      if (event.type === 'ThemeChanged') {
        // SetTheme is handled inside slash command
      }
    });

    const unsubscribeFocus = {
      unsubscribe: () => {}
    };

    // Track active widget ID to highlight borders
    const changeUnsubscribe = (FocusManager as any).onChange((active: any) => {
      const activeId = active ? active.id : null;
      setActiveWidgetId(activeId);
      if (activeId === 'file-browser' || activeId === 'config-wizard') {
        setActiveTab(activeId);
      }
    });

    // Ensure prompt-input starts focused
    FocusManager.focusWidget('prompt-input');

    return () => {
      unsubscribe();
      changeUnsubscribe();
    };
  }, []);

  // Listen to UDS client messages
  useEffect(() => {
    client.connect();

    const unsubscribeLog = client.onLog((message) => {
      addLog(`[CLI Log] ${message}`);
    });

    const unsubscribeMsg = client.onMessage((msg: ServerResponse) => {
      const handleNonStreaming = (status: string, body: string) => {
        setIsLoading(false);
        EventBus.publish({ type: 'QueryFinished', success: status === 'ok' || status === 'success' });
        setSessionStats((prev) => ({
          inputTokens: prev.inputTokens + Math.floor(Math.random() * 200) + 100,
          outputTokens: prev.outputTokens + Math.floor(Math.random() * 100) + 50,
          cost: prev.cost + 0.0004,
        }));
        
        // If there was a previous stream rendering, commit it to logs first
        if (isStreamingRef.current && displayedTextRef.current) {
          addLog(displayedTextRef.current);
        }

        startStream('legacy-stream');
        queueChunk(`[Daemon Response] Status: ${status.toUpperCase()}\n${body}`, 1);
        endStream(2);
      };

      if (msg && typeof msg === 'object' && 'type' in msg) {
        const eventType = msg.type;
        switch (eventType) {
          case 'stream_start': {
            if (isStreamingRef.current && displayedTextRef.current) {
              addLog(displayedTextRef.current);
            }
            startStream(msg.streamId);
            break;
          }
          case 'stream_progress': {
            handleProgress(msg.progress, msg.message, msg.sequence);
            break;
          }
          case 'stream_chunk': {
            queueChunk(msg.content, msg.sequence);
            break;
          }
          case 'stream_end': {
            setIsLoading(false);
            EventBus.publish({ type: 'QueryFinished', success: true });
            endStream(msg.sequence);
            break;
          }
          case 'stream_cancelled': {
            setIsLoading(false);
            EventBus.publish({ type: 'QueryFinished', success: false });
            cancelStream(msg.sequence);
            break;
          }
          case 'Response': {
            handleNonStreaming(msg.status, msg.body);
            break;
          }
          case 'Error': {
            handleNonStreaming(msg.status, msg.body);
            break;
          }
          default: {
            const streamId = (msg as any).streamId;
            const warningMsg = streamId 
              ? `[Protocol Warning] Ignored unknown stream event "${eventType}" for stream "${streamId}"`
              : `[Protocol Warning] Ignored unknown stream event "${eventType}"`;
            addLog(warningMsg);
            break;
          }
        }
      } else if (msg && typeof msg === 'object') {
        handleNonStreaming(msg.status, msg.message);
      }
    });

    return () => {
      unsubscribeLog();
      unsubscribeMsg();
    };
  }, [client]);

  // When streaming finishes, commit to permanent log store
  useEffect(() => {
    if (!isStreaming && displayedText) {
      addLog(displayedText);
    }
  }, [isStreaming]);

  // Process command submission
  const handleCommandSubmit = async (command: string) => {
    const trimmed = command.trim();
    if (!trimmed) return;

    // Add prompt text to scrollback immediately
    addLog(`> ${trimmed}`);
    EventBus.publish({ type: 'HistoryAdded', command: trimmed });

    // Handle exit/quit commands
    if (trimmed.toLowerCase() === 'exit' || trimmed.toLowerCase() === 'quit') {
      addLog('[System] Exiting client REPL...');
      setTimeout(() => process.exit(0), 400);
      return;
    }

    // 1. Run local slash commands first
    const isSlash = await executeCommand(trimmed, {
      setLogs,
      setTheme,
      client,
    });

    if (isSlash) return;

    // 2. Otherwise, route action query/ingest to UDS Daemon socket
    setIsLoading(true);
    EventBus.publish({ type: 'QueryStarted', query: trimmed });

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

  // Intercept key inputs to toggle launcher palette and handle Tab key cycling
  useInput((input, key) => {
    if (key.ctrl && input === 'p') {
      setPaletteOpen((prev) => !prev);
      return;
    }

    if (input === '\t' || key.tab) {
      if (activeWidgetId === 'prompt-input') {
        if (activeTab !== 'file-browser') {
          setActiveTab('file-browser');
          setShouldFocusTab(true);
        }
      } else if (activeWidgetId === 'file-browser') {
        setActiveTab('config-wizard');
        setShouldFocusTab(true);
      } else if (activeWidgetId === 'config-wizard') {
        FocusManager.focusWidget('prompt-input');
      }
    }
  });

  const displayLogs = logs.slice(-10); // Keep last 10 logs to fit layout

  return (
    <ThemedBox flexDirection="column" padding={1} width="100%" height="100%" backgroundColor="clawd_background">
      {/* Wordmark Logo */}
      <LogoV2 />

      <Divider title="Relational Memory Dashboard" color="subtle" />

      {/* Main workspace layout */}
      <ThemedBox flexDirection={isWide ? 'row' : 'column'} width="100%">
        {/* Left Side: Logs Terminal */}
        <ThemedBox flexDirection="column" width={isWide ? '68%' : '100%'} marginRight={isWide ? 2 : 0}>
          <ThemedBox
            flexDirection="column"
            borderStyle="single"
            borderColor={activeWidgetId === 'prompt-input' ? 'claude' : 'promptBorder'}
            padding={1}
            minHeight={12}
            flexGrow={1}
            marginTop={1}
          >
            {displayLogs.length === 0 && !isStreaming ? (
              <ThemedText color="inactive" italic>
                {"Memory Stream Terminal is ready. Send commands format: \"query <text>\" or \"ingest <text>\""}
              </ThemedText>
            ) : (
              <ThemedBox flexDirection="column">
                {displayLogs.map((log, index) => {
                  let color = 'text';
                  if (log.startsWith('> ')) color = 'success';
                  else if (log.includes('[CLI Log] [Error]')) color = 'error';
                  else if (log.includes('[CLI Log]')) color = 'warning';
                  else if (log.includes('[System]')) color = 'suggestion';

                  return (
                    <ThemedBox key={index} marginBottom={0}>
                      {log.startsWith('>') || log.startsWith('[CLI Log]') || log.startsWith('[System]') ? (
                        <ThemedText color={color as any}>{log}</ThemedText>
                      ) : (
                        <MarkdownRenderer nodes={parseMarkdown(log)} />
                      )}
                    </ThemedBox>
                  );
                })}
                {/* Simulated live typewriter stream */}
                {isStreaming && (
                  <ThemedBox marginTop={1}>
                    <MarkdownRenderer nodes={parseMarkdown(displayedText)} />
                  </ThemedBox>
                )}
              </ThemedBox>
            )}
          </ThemedBox>
        </ThemedBox>

        {/* Right Side: Real-time Monitor Panel & Interactive Widgets */}
        <ThemedBox flexDirection="column" width={isWide ? '30%' : '100%'} marginTop={isWide ? 0 : 1}>
          {/* Metrics Panel */}
          {isConnected && metrics ? (
            <WidgetContainer isFocused={false}>
              <WidgetHeader title="System Monitor" isFocused={false} />
              <WidgetBody>
                <ThemedBox flexDirection="column">
                  <ThemedBox flexDirection="row" justifyContent="space-between">
                    <ThemedText color="inactive">Cache Hit Rate:</ThemedText>
                    <ThemedText color="success" bold>{(metrics.cache_hit_rate * 100).toFixed(1)}%</ThemedText>
                  </ThemedBox>
                  <ThemedBox flexDirection="row" justifyContent="space-between">
                    <ThemedText color="inactive">Total Queries:</ThemedText>
                    <ThemedText color="text" bold>{metrics.total_queries}</ThemedText>
                  </ThemedBox>
                  <ThemedBox flexDirection="row" justifyContent="space-between">
                    <ThemedText color="inactive">Total Ingests:</ThemedText>
                    <ThemedText color="text" bold>{metrics.total_ingests}</ThemedText>
                  </ThemedBox>
                  <ThemedBox flexDirection="row" justifyContent="space-between">
                    <ThemedText color="inactive">Queue Depth:</ThemedText>
                    <ThemedText color="professionalBlue" bold>{metrics.queue_depth}</ThemedText>
                  </ThemedBox>
                  <ThemedBox flexDirection="row" justifyContent="space-between">
                    <ThemedText color="inactive">Active Theme:</ThemedText>
                    <ThemedText color="chromeYellow" bold>{themeType}</ThemedText>
                  </ThemedBox>
                </ThemedBox>
              </WidgetBody>
            </WidgetContainer>
          ) : (
            <WidgetContainer isFocused={false}>
              <WidgetHeader title="System Monitor" />
              <WidgetBody state="error" errorMessage="Daemon stats server unreachable." />
            </WidgetContainer>
          )}

          {/* Interactive widgets panel */}
          <ThemedBox flexDirection="column" marginTop={1}>
            {/* Tab Headers */}
            <ThemedBox flexDirection="row" gap={2} marginBottom={1} paddingX={1}>
              <ThemedText
                color={activeTab === 'file-browser' ? 'claude' : 'inactive'}
                bold
                underline={activeTab === 'file-browser'}
              >
                {activeWidgetId === 'file-browser' ? '● File Browser' : '  File Browser'}
              </ThemedText>
              <ThemedText color="inactive">|</ThemedText>
              <ThemedText
                color={activeTab === 'config-wizard' ? 'claude' : 'inactive'}
                bold
                underline={activeTab === 'config-wizard'}
              >
                {activeWidgetId === 'config-wizard' ? '● Config Wizard' : '  Config Wizard'}
              </ThemedText>
            </ThemedBox>

            <FileBrowser isFocused={activeWidgetId === 'file-browser'} visible={activeTab === 'file-browser'} />
            <MultiStepForm visible={activeTab === 'config-wizard'} />
          </ThemedBox>
        </ThemedBox>
      </ThemedBox>

      {/* Command Palette Overlay */}
      {paletteOpen && (
        <CommandPalette
          onClose={() => setPaletteOpen(false)}
          context={{ setLogs, setTheme, client }}
        />
      )}

      {/* Status Bar */}
      <ThemedBox flexDirection="row" justifyContent="space-between" paddingX={1} marginY={1}>
        <ThemedBox flexDirection="row" alignItems="center">
          <ThemedText color="claude" bold>Daemon Status: </ThemedText>
          {isLoading ? (
            <ThemedText color="chromeYellow" bold>● Processing...</ThemedText>
          ) : isStreaming && progress ? (
            <ThemedText color="chromeYellow" bold>{`● ${progress.message} (${Math.round(progress.progress * 100)}%)`}</ThemedText>
          ) : (
            <ThemedText color={isConnected ? 'success' : 'error'} bold>
              {isConnected ? '● Connected' : '✗ Unreachable'}
            </ThemedText>
          )}
          <ThemedText color="inactive" marginLeft={3}>
            Press [Tab] to cycle focus | Press [Ctrl+P] to toggle Command Launcher
          </ThemedText>
        </ThemedBox>
      </ThemedBox>

      {/* Keystroke Entry Panel */}
      <ThemedBox borderStyle="single" borderColor={activeWidgetId === 'prompt-input' ? 'claude' : 'promptBorder'} paddingX={1} marginBottom={1}>
        <PromptInput onSubmit={handleCommandSubmit} />
      </ThemedBox>

      {/* footer status line */}
      <StatusLine
        mode="Auto"
        modelName="Gemini 3.5 Flash"
        tokens={{ input: sessionStats.inputTokens, output: sessionStats.outputTokens }}
        cost={sessionStats.cost}
        rateLimitPercent={8}
      />
    </ThemedBox>
  );
};
