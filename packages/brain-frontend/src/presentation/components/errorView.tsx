import { Text } from 'ink';

import type { BrainTheme } from '../theme.js';

export function ErrorView(props: Readonly<{ message: string; theme: BrainTheme }>) {
  return <Text color={props.theme.error}>Error: {props.message}</Text>;
}
