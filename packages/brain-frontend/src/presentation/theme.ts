/** A single owned semantic terminal palette; customization is intentionally deferred. */
export type BrainTheme = Readonly<{
  text: string;
  muted: string;
  accent: string;
  error: string;
  warning: string;
  success: string;
  border: string;
}>;

export const defaultBrainTheme: BrainTheme = Object.freeze({
  text: 'white',
  muted: 'gray',
  accent: 'cyan',
  error: 'red',
  warning: 'yellow',
  success: 'green',
  border: 'gray',
});
