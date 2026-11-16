export type FrontendFocus = 'composer' | 'session-picker' | 'permission' | 'context';

export function composerFocus(): FrontendFocus {
  return 'composer';
}

export function sessionPickerFocus(): FrontendFocus {
  return 'session-picker';
}
