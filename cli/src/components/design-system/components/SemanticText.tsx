import React from 'react';
import { ThemedText, ThemedTextProps } from './ThemedText';
import { ThemedBox } from './ThemedBox';

// Higher-level semantic text components
export const SuccessText: React.FC<ThemedTextProps> = (props) => (
  <ThemedText color="success" {...props} />
);

export const WarningText: React.FC<ThemedTextProps> = (props) => (
  <ThemedText color="warning" {...props} />
);

export const ErrorText: React.FC<ThemedTextProps> = (props) => (
  <ThemedText color="error" {...props} />
);

export const MutedText: React.FC<ThemedTextProps> = (props) => (
  <ThemedText color="inactive" {...props} />
);

// Helper badge component
interface BadgeProps {
  children: React.ReactNode;
}

export const SuccessBadge: React.FC<BadgeProps> = ({ children }) => (
  <ThemedBox backgroundColor="success" paddingX={1}>
    <ThemedText color="inverseText" bold>
      {children}
    </ThemedText>
  </ThemedBox>
);

export const WarningBadge: React.FC<BadgeProps> = ({ children }) => (
  <ThemedBox backgroundColor="warning" paddingX={1}>
    <ThemedText color="inverseText" bold>
      {children}
    </ThemedText>
  </ThemedBox>
);

// Typo: TitleBox -> ThemedBox (using it in ErrorBadge/WarningBadge)
export const ErrorBadge: React.FC<BadgeProps> = ({ children }) => (
  <ThemedBox backgroundColor="error" paddingX={1}>
    <ThemedText color="inverseText" bold>
      {children}
    </ThemedText>
  </ThemedBox>
);
