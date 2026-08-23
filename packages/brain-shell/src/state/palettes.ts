/**
 * Brain-owned color palettes. Original values — no external theme source.
 * Roles mirror BrainThemeTokens so the adapter can delegate here directly.
 */
import type { ThemeName } from '../contracts/theme.js';

export interface BrainTokens {
  // Brand & High-Priority Accents
  brand: string;
  brandShimmer: string;
  accent: string;
  // Surface & Layout Frames
  promptBorder: string;
  promptBorderInactive: string;
  subtle: string;
  background: string;
  userBackground: string;
  // Typography
  text: string;
  inverseText: string;
  muted: string;
  // Status & Telemetry
  success: string;
  error: string;
  warning: string;
  // Diffs
  diffAdded: string;
  diffRemoved: string;
  diffAddedWord: string;
  diffRemovedWord: string;
  // Interaction
  selectionBg: string;
}

export const PALETTES: Record<ThemeName, BrainTokens> = {
  dark: {
    brand: '#D97757',
    brandShimmer: '#E8987A',
    accent: '#82A8FF',
    promptBorder: '#5C5348',
    promptBorderInactive: '#3A342C',
    subtle: '#3A342C',
    background: '#1A1714',
    userBackground: '#242019',
    text: '#EDE6DF',
    inverseText: '#1A1714',
    muted: '#8C8478',
    success: '#5CB87A',
    error: '#E5554B',
    warning: '#D9A03F',
    diffAdded: '#20301F',
    diffRemoved: '#331F1D',
    diffAddedWord: '#3E6B39',
    diffRemovedWord: '#7C3A32',
    selectionBg: '#3A4054',
  },
  light: {
    brand: '#C25E3F',
    brandShimmer: '#D97B5C',
    accent: '#3B66C4',
    promptBorder: '#B9AFA2',
    promptBorderInactive: '#D8D0C5',
    subtle: '#E7E0D6',
    background: '#FAF6F0',
    userBackground: '#F0E9DF',
    text: '#2B2620',
    inverseText: '#FAF6F0',
    muted: '#7A7166',
    success: '#2E7D4F',
    error: '#C23B32',
    warning: '#9A6B1F',
    diffAdded: '#DDF0DC',
    diffRemoved: '#F6DEDB',
    diffAddedWord: '#A8D8A2',
    diffRemovedWord: '#EBA69E',
    selectionBg: '#CBD5EE',
  },
  'dark-daltonized': {
    brand: '#D97757',
    brandShimmer: '#E8987A',
    accent: '#6FB7E8',
    promptBorder: '#565058',
    promptBorderInactive: '#38343C',
    subtle: '#38343C',
    background: '#191720',
    userBackground: '#232029',
    text: '#EAE6EC',
    inverseText: '#191720',
    muted: '#8E8896',
    success: '#4E9EB8',
    error: '#E08A3C',
    warning: '#C7B44A',
    diffAdded: '#1E2E36',
    diffRemoved: '#37281C',
    diffAddedWord: '#35647A',
    diffRemovedWord: '#A06428',
    selectionBg: '#31445E',
  },
  'light-daltonized': {
    brand: '#C25E3F',
    brandShimmer: '#D97B5C',
    accent: '#2E6EA6',
    promptBorder: '#AEAAB4',
    promptBorderInactive: '#D5D2DA',
    subtle: '#E6E4EA',
    background: '#F7F6FA',
    userBackground: '#ECEAF2',
    text: '#28242E',
    inverseText: '#F7F6FA',
    muted: '#75707E',
    success: '#22708C',
    error: '#B06A1E',
    warning: '#8A7A18',
    diffAdded: '#DBEDF5',
    diffRemoved: '#F5E6D4',
    diffAddedWord: '#A3CFE3',
    diffRemovedWord: '#E4BC85',
    selectionBg: '#CCD8EC',
  },
};
