export type WidgetState = 'idle' | 'loading' | 'error' | 'searching' | 'selecting';

export interface InteractiveWidget {
  id: string;
  title: string;
  handleInput(input: string, key: {
    upArrow: boolean;
    downArrow: boolean;
    leftArrow: boolean;
    rightArrow: boolean;
    return: boolean;
    escape: boolean;
    backspace: boolean;
    delete: boolean;
    ctrl: boolean;
    meta: boolean;
  }): boolean;
  onFocus?(): void;
  onBlur?(): void;
  onMount?(): void;
}
