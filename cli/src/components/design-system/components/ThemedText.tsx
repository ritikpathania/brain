import React from 'react';
import { Text, TextProps } from 'ink';
import { useTheme } from '../hooks';
import { ColorToken, TypographyToken } from '../tokens';

export interface ThemedTextProps extends Omit<TextProps, 'color' | 'backgroundColor'> {
  color?: ColorToken | string;
  backgroundColor?: ColorToken | string;
  variant?: TypographyToken;
  children?: React.ReactNode;
}

export const ThemedText: React.FC<ThemedTextProps> = ({
  color: colorProp,
  backgroundColor: bgProp,
  variant,
  children,
  ...textProps
}) => {
  const { color, typography } = useTheme();

  // Resolve colors using helper API
  const resolvedColor = colorProp ? color(colorProp) : undefined;
  const resolvedBgColor = bgProp ? color(bgProp) : undefined;

  // Resolve typography styling
  const typoStyle = variant ? typography(variant) : {};
  const isBold = textProps.bold ?? typoStyle.bold;
  const isUnderline = textProps.underline ?? typoStyle.underline;
  const isDimColor = textProps.dimColor ?? typoStyle.dimColor;
  const isInverse = textProps.inverse ?? typoStyle.inverse;

  return (
    <Text
      color={resolvedColor}
      backgroundColor={resolvedBgColor}
      bold={isBold}
      underline={isUnderline}
      dimColor={isDimColor}
      inverse={isInverse}
      {...textProps}
    >
      {children}
    </Text>
  );
};
