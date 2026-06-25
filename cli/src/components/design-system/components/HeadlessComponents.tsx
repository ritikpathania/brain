import React, { useState, useEffect } from 'react';
import { ThemedBox } from './ThemedBox';
import { ThemedText } from './ThemedText';
import { Panel } from './ThemedView';
import { useTheme } from '../hooks';
import { ColorToken, IconToken } from '../tokens';

// 1. Headless Alert Component
interface AlertProps {
  severity: 'success' | 'warning' | 'error' | 'info';
  title?: string;
  children: React.ReactNode;
}

export const Alert: React.FC<AlertProps> = ({ severity, title, children }) => {
  const { icon } = useTheme();

  // Map severity to semantic colors
  let color: ColorToken = 'text';
  if (severity === 'success') color = 'success';
  if (severity === 'warning') color = 'warning';
  if (severity === 'error') color = 'error';
  if (severity === 'info') color = 'suggestion';

  const severityIcon = icon(severity as IconToken);

  return (
    <Panel
      borderColor={color}
      title={title || severity.toUpperCase()}
      titleColor={color}
      padding={1}
      borderStyle="single"
    >
      <ThemedBox flexDirection="row" alignItems="center">
        <ThemedText color={color} bold marginRight={1}>
          {severityIcon}
        </ThemedText>
        <ThemedBox flexShrink={1}>
          <ThemedText color="text">{children}</ThemedText>
        </ThemedBox>
      </ThemedBox>
    </Panel>
  );
};

// 2. Headless Progress Bar Component
interface ProgressProps {
  percent: number; // 0 to 100
  width?: number; // width in character columns
}

export const Progress: React.FC<ProgressProps> = ({ percent, width = 20 }) => {
  const clamped = Math.max(0, Math.min(100, percent));
  const filledWidth = Math.round((clamped / 100) * width);
  const emptyWidth = width - filledWidth;

  return (
    <ThemedBox flexDirection="row" alignItems="center">
      <ThemedText color="rate_limit_fill">
        {'█'.repeat(filledWidth)}
      </ThemedText>
      <ThemedText color="rate_limit_empty">
        {'░'.repeat(emptyWidth)}
      </ThemedText>
      <ThemedText color="inactive" marginLeft={1}>
        {clamped}%
      </ThemedText>
    </ThemedBox>
  );
};

// 3. Headless Toast Notification Component
interface ToastProps {
  message: string;
  duration?: number; // in milliseconds
  onClose?: () => void;
}

export const Toast: React.FC<ToastProps> = ({ message, duration = 3000, onClose }) => {
  const [visible, setVisible] = useState(true);

  useEffect(() => {
    const timer = setTimeout(() => {
      setVisible(false);
      if (onClose) onClose();
    }, duration);

    return () => clearTimeout(timer);
  }, [duration, onClose]);

  if (!visible) return null;

  return (
    <ThemedBox
      borderStyle="round"
      borderColor="suggestion"
      backgroundColor="messageActionsBackground"
      paddingX={2}
      paddingY={0}
      alignSelf="center"
    >
      <ThemedText color="text" bold>
        {message}
      </ThemedText>
    </ThemedBox>
  );
};
