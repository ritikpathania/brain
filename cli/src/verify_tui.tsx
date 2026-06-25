import React, { useState } from 'react';
import { render, useInput } from 'ink';
import {
  ThemeProvider,
  ThemeType,
  useTheme,
  LogoV2,
  Divider,
  Panel,
  Card,
  Spinner,
  StatusLine,
  Alert,
  Progress,
  SuccessText,
  WarningText,
  ErrorText,
  MutedText,
  SuccessBadge,
  WarningBadge,
  ErrorBadge,
  ThemedText,
  ThemedBox,
  Toast,
} from './components/design-system';

export const themesList: ThemeType[] = [
  'dark',
  'light',
  'dark-daltonized',
  'light-daltonized',
  'dark-ansi',
  'light-ansi',
];

// 1. Theme Scenario Component
export const ThemeScenario: React.FC = () => {
  const { themeType } = useTheme();
  return (
    <Card marginTop={1}>
      <MutedText>
        Current Theme: <ThemedText color="claude" bold>{themeType.toUpperCase()}</ThemedText>
      </MutedText>
    </Card>
  );
};

// 2. Alert Scenario Component
export const AlertScenario: React.FC = () => {
  return (
    <Panel borderStyle="single" borderColor="promptBorder" marginTop={1}>
      <Alert severity="success" title="SYSTEM STATUS">
        Connection to database has been established successfully.
      </Alert>
      <ThemedBox marginTop={1}>
        <Alert severity="error" title="CRITICAL FAULT">
          Python PyO3 FFI interpreter failed to bind port.
        </Alert>
      </ThemedBox>

      <ThemedBox flexDirection="row" gap={3} marginTop={1} alignItems="center">
        <SuccessText>Success Label</SuccessText>
        <SuccessBadge>OK</SuccessBadge>
        <WarningText>Warning Label</WarningText>
        <WarningBadge>WARN</WarningBadge>
        <ErrorText>Error Label</ErrorText>
        <ErrorBadge>FAIL</ErrorBadge>
      </ThemedBox>
    </Panel>
  );
};

// 3. Spinner Scenario Component
export const SpinnerScenario: React.FC = () => {
  return (
    <ThemedBox flexDirection="column" gap={1} marginY={1}>
      <Spinner label="Pulsing shimmer breathing (claude/shimmer)..." />
      <Spinner label="Loading model weights..." />
    </ThemedBox>
  );
};

// 4. Resize Scenario Component
interface ResizeScenarioProps {
  width: number;
}
export const ResizeScenario: React.FC<ResizeScenarioProps> = ({ width }) => {
  return (
    <ThemedBox width={width} borderStyle="single" borderColor="inactive" padding={1} flexDirection="column">
      <ThemedText color="text" bold>Resize Scenario (Width: {width} columns)</ThemedText>
      <ThemedBox flexDirection="row" gap={2} marginTop={1}>
        <ThemedBox flexGrow={1} borderStyle="round" borderColor="claude" padding={1}>
          <ThemedText color="text">Flex Grow Content A</ThemedText>
        </ThemedBox>
        {width >= 60 && (
          <ThemedBox flexGrow={1} borderStyle="round" borderColor="suggestion" padding={1}>
            <ThemedText color="text">Flex Grow Content B (Hidden when width &lt; 60)</ThemedText>
          </ThemedBox>
        )}
      </ThemedBox>
    </ThemedBox>
  );
};

// 5. Toast Scenario Component
interface ToastScenarioProps {
  showToast: boolean;
  message?: string;
}
export const ToastScenario: React.FC<ToastScenarioProps> = ({
  showToast,
  message = "Toast Notification Triggered!",
}) => {
  return (
    <ThemedBox flexDirection="column" marginY={1} minHeight={3}>
      <ThemedText color="inactive">Toast Scenario Box:</ThemedText>
      {showToast && <Toast message={message} duration={100000} />}
    </ThemedBox>
  );
};

// 6. History Scenario Component
interface HistoryScenarioProps {
  index: number;
}
export const HistoryScenario: React.FC<HistoryScenarioProps> = ({ index }) => {
  const history = [
    'ingest first item',
    'query database',
    'ingest second item',
    'query postgres',
  ];

  return (
    <ThemedBox flexDirection="column" padding={1} borderStyle="round" borderColor="claude">
      <ThemedText color="text" bold>Command History Navigation</ThemedText>
      {history.map((cmd, idx) => (
        <ThemedText key={idx} color={idx === index ? 'claude' : 'inactive'}>
          {idx === index ? '➔ ' : '  '} {cmd}
        </ThemedText>
      ))}
    </ThemedBox>
  );
};

// Main Verification Application wrapping all scenarios
export const VerificationApp: React.FC = () => {
  const { themeType, setTheme } = useTheme();
  const [percent, setPercent] = useState(35);
  const [showToast, setShowToast] = useState(false);
  const [historyIndex, setHistoryIndex] = useState(0);
  const [terminalWidth, setTerminalWidth] = useState(80);

  // Cycle progress percentage for animation testing
  React.useEffect(() => {
    const timer = setInterval(() => {
      setPercent((prev) => (prev + 5 > 100 ? 0 : prev + 5));
    }, 500);
    return () => clearInterval(timer);
  }, []);

  // Listen to keyboard inputs
  useInput((input, key) => {
    if (input === 'q' || key.escape) {
      process.exit(0);
    }
    if (input === 't') {
      const idx = themesList.indexOf(themeType);
      const nextIdx = (idx + 1) % themesList.length;
      setTheme(themesList[nextIdx]);
    }
    if (input === 's') {
      setShowToast((prev) => !prev);
    }
    if (key.upArrow) {
      setHistoryIndex((prev) => (prev > 0 ? prev - 1 : 3));
    }
    if (key.downArrow) {
      setHistoryIndex((prev) => (prev < 3 ? prev + 1 : 0));
    }
    if (key.leftArrow) {
      setTerminalWidth((prev) => Math.max(40, prev - 10));
    }
    if (key.rightArrow) {
      setTerminalWidth((prev) => Math.min(120, prev + 10));
    }
  });

  return (
    <Panel borderStyle="double" borderColor="claude" padding={1} width="100%">
      <LogoV2 />

      <Divider title="Manual Verification Harness" color="subtle" />

      {/* Instructions */}
      <Card marginTop={1}>
        <MutedText>
          Press <ThemedText color="claude" bold>t</ThemedText> to cycle themes · Current Theme: <ThemedText color="claude" bold>{themeType.toUpperCase()}</ThemedText>
        </MutedText>
        <MutedText>
          Press <ThemedText color="suggestion" bold>s</ThemedText> to toggle toast notification
        </MutedText>
        <MutedText>
          Press <ThemedText color="claude" bold>↑ / ↓</ThemedText> to navigate command history
        </MutedText>
        <MutedText>
          Press <ThemedText color="claude" bold>← / →</ThemedText> to resize layout width ({terminalWidth} cols)
        </MutedText>
        <MutedText>
          Press <ThemedText color="error" bold>q</ThemedText> or <ThemedText color="error" bold>Esc</ThemedText> to quit verification harness
        </MutedText>
      </Card>

      <ThemeScenario />

      <Divider title="Alert & Badges Scenario" color="subtle" />
      <AlertScenario />

      <Divider title="Toast Notification Scenario" color="subtle" />
      <ToastScenario showToast={showToast} />

      <Divider title="Spinner Scenario" color="subtle" />
      <SpinnerScenario />

      <Divider title="Command History Scenario" color="subtle" />
      <HistoryScenario index={historyIndex} />

      <Divider title="Resize Scenario" color="subtle" />
      <ResizeScenario width={terminalWidth} />

      {/* Status Pinned Footer */}
      <StatusLine
        mode="Plan"
        modelName="Gemini 3.5 Flash"
        tokens={{ input: 2450, output: 890 }}
        cost={0.0125}
        rateLimitPercent={percent}
      />
    </Panel>
  );
};

const runHarness = () => {
  render(
    <ThemeProvider>
      <VerificationApp />
    </ThemeProvider>
  );
};

if (import.meta.main) {
  runHarness();
}
