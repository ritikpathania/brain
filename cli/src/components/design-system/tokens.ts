export type ColorToken =
  // Brand & Accent
  | 'primary'
  | 'claude'
  | 'claudeShimmer'
  | 'claudeBlue_FOR_SYSTEM_SPINNER'
  | 'claudeBlueShimmer_FOR_SYSTEM_SPINNER'
  | 'autoAccept'
  | 'bashBorder'
  | 'permission'
  | 'permissionShimmer'
  | 'planMode'
  | 'ide'
  | 'fastMode'
  | 'fastModeShimmer'
  // Chrome & UI
  | 'promptBorder'
  | 'promptBorderShimmer'
  | 'text'
  | 'inverseText'
  | 'inactive'
  | 'inactiveShimmer'
  | 'subtle'
  | 'suggestion'
  | 'remember'
  | 'background'
  | 'merged'
  | 'chromeYellow'
  | 'professionalBlue'
  // Semantic
  | 'success'
  | 'error'
  | 'warning'
  | 'warningShimmer'
  // Diff
  | 'diffAdded'
  | 'diffRemoved'
  | 'diffAddedDimmed'
  | 'diffRemovedDimmed'
  | 'diffAddedWord'
  | 'diffRemovedWord'
  // Surfaces / Backgrounds
  | 'userMessageBackground'
  | 'userMessageBackgroundHover'
  | 'messageActionsBackground'
  | 'selectionBg'
  | 'bashMessageBackgroundColor'
  | 'memoryBackgroundColor'
  | 'clawd_body'
  | 'clawd_background'
  // Rate Limit
  | 'rate_limit_fill'
  | 'rate_limit_empty'
  // Brief / Assistant Mode Labels
  | 'briefLabelYou'
  | 'briefLabelClaude'
  // Subagents
  | 'subagent-1'
  | 'subagent-2'
  | 'subagent-3'
  | 'subagent-4'
  | 'subagent-5'
  | 'subagent-6'
  | 'subagent-7'
  | 'subagent-8'
  // Rainbow Keywords (Ultrathink)
  | 'rainbow-1'
  | 'rainbow-2'
  | 'rainbow-3'
  | 'rainbow-4'
  | 'rainbow-5'
  | 'rainbow-6'
  | 'rainbow-7'
  | 'rainbow-1-shimmer'
  | 'rainbow-2-shimmer'
  | 'rainbow-3-shimmer'
  | 'rainbow-4-shimmer'
  | 'rainbow-5-shimmer'
  | 'rainbow-6-shimmer'
  | 'rainbow-7-shimmer';

export type TypographyToken = 'headline' | 'body' | 'label';

export type IconToken = 'success' | 'warning' | 'error' | 'info' | 'pending';

export type BorderToken = 'style';

export type SpacingToken = 'none' | 'tight' | 'normal' | 'relaxed' | 'section';

export interface TypographyStyle {
  bold?: boolean;
  underline?: boolean;
  dimColor?: boolean;
  inverse?: boolean;
}

export type ThemeColors = Record<ColorToken, string>;
export type ThemeSpacing = Record<SpacingToken, number>;
export type ThemeBorders = Record<BorderToken, 'single' | 'double' | 'round' | 'classic'>;
export type ThemeIcons = Record<IconToken, string>;
export type ThemeTypography = Record<TypographyToken, TypographyStyle>;

export interface Theme {
  colors: ThemeColors;
  spacing: ThemeSpacing;
  borders: ThemeBorders;
  icons: ThemeIcons;
  typography: ThemeTypography;
}
