import React from 'react';
import { ThemedBox, ThemedText, Spinner } from '../../design-system';
import { WidgetState } from './InteractiveWidget';

interface WidgetContainerProps {
  isFocused?: boolean;
  flexDirection?: 'row' | 'column';
  height?: number | string;
  minHeight?: number;
  flexGrow?: number;
  flexShrink?: number;
  width?: number | string;
  marginTop?: number;
  marginBottom?: number;
  children: React.ReactNode;
}

export const WidgetContainer: React.FC<WidgetContainerProps> = ({
  isFocused = false,
  flexDirection = 'column',
  height,
  minHeight,
  flexGrow,
  flexShrink,
  width,
  marginTop,
  marginBottom,
  children,
}) => {
  return (
    <ThemedBox
      flexDirection={flexDirection}
      borderStyle="round"
      borderColor={isFocused ? 'claude' : 'promptBorder'}
      padding={1}
      height={height}
      minHeight={minHeight}
      flexGrow={flexGrow}
      flexShrink={flexShrink}
      width={width}
      marginTop={marginTop}
      marginBottom={marginBottom}
    >
      {children}
    </ThemedBox>
  );
};

interface WidgetHeaderProps {
  title: string;
  isFocused?: boolean;
  state?: WidgetState;
  errorMessage?: string;
}

export const WidgetHeader: React.FC<WidgetHeaderProps> = ({
  title,
  isFocused = false,
  state = 'idle',
  errorMessage,
}) => {
  let stateTag = null;
  if (state === 'loading') {
    stateTag = <ThemedText color="claudeBlue_FOR_SYSTEM_SPINNER" bold>[LOADING]</ThemedText>;
  } else if (state === 'searching') {
    stateTag = <ThemedText color="professionalBlue" bold>[SEARCHING]</ThemedText>;
  } else if (state === 'selecting') {
    stateTag = <ThemedText color="chromeYellow" bold>[SELECTING]</ThemedText>;
  } else if (state === 'error') {
    stateTag = <ThemedText color="error" bold>[ERROR]</ThemedText>;
  }

  return (
    <ThemedBox flexDirection="row" justifyContent="space-between" marginBottom={1}>
      <ThemedBox flexDirection="row">
        <ThemedText color={isFocused ? 'claude' : 'text'} bold>
          {isFocused ? '● ' : '○ '}
          {title}
        </ThemedText>
      </ThemedBox>
      {stateTag}
    </ThemedBox>
  );
};

interface WidgetBodyProps {
  state?: WidgetState;
  errorMessage?: string;
  loadingLabel?: string;
  children: React.ReactNode;
}

export const WidgetBody: React.FC<WidgetBodyProps> = ({
  state = 'idle',
  errorMessage,
  loadingLabel = 'Loading data...',
  children,
}) => {
  if (state === 'loading') {
    return (
      <ThemedBox flexGrow={1} justifyContent="center" alignItems="center" minHeight={3}>
        <Spinner label={loadingLabel} />
      </ThemedBox>
    );
  }

  if (state === 'error') {
    return (
      <ThemedBox flexGrow={1} padding={1} minHeight={3}>
        <ThemedText color="error" bold>
          Error: {errorMessage || 'An unknown error occurred.'}
        </ThemedText>
      </ThemedBox>
    );
  }

  return <ThemedBox flexGrow={1}>{children}</ThemedBox>;
};

interface WidgetFooterProps {
  shortcuts?: { key: string; description: string }[];
  statusText?: string;
}

export const WidgetFooter: React.FC<WidgetFooterProps> = ({ shortcuts, statusText }) => {
  if (!shortcuts && !statusText) return null;

  return (
    <ThemedBox
      flexDirection="row"
      justifyContent="space-between"
      marginTop={1}
      borderStyle="classic"
      borderColor="subtle"
      paddingTop={1}
    >
      <ThemedBox flexDirection="row" flexWrap="wrap">
        {shortcuts?.map((s, idx) => (
          <ThemedText key={idx} color="inactive" marginRight={2}>
            <ThemedText color="primary" bold>{s.key}</ThemedText>: {s.description}
          </ThemedText>
        ))}
      </ThemedBox>
      {statusText && (
        <ThemedText color="inactive" italic>
          {statusText}
        </ThemedText>
      )}
    </ThemedBox>
  );
};

interface WidgetEmptyStateProps {
  message?: string;
}

export const WidgetEmptyState: React.FC<WidgetEmptyStateProps> = ({
  message = 'No entries found.',
}) => {
  return (
    <ThemedBox flexGrow={1} justifyContent="center" alignItems="center" padding={2} minHeight={4}>
      <ThemedText color="inactive" italic>
        {message}
      </ThemedText>
    </ThemedBox>
  );
};
