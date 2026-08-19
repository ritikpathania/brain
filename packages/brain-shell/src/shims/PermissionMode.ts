import { feature } from 'bun:bundle';
import { z } from 'zod';
import { PAUSE_ICON } from '../../vendor/claude/constants/figures.js';
import {
  EXTERNAL_PERMISSION_MODES,
  type ExternalPermissionMode,
  PERMISSION_MODES,
  type PermissionMode,
} from '../../vendor/claude/types/permissions.js';
import { lazySchema } from '../../vendor/claude/utils/lazySchema.js';

export {
  EXTERNAL_PERMISSION_MODES,
  PERMISSION_MODES,
  type ExternalPermissionMode,
  type PermissionMode,
};

export const permissionModeSchema = lazySchema(() => z.enum(PERMISSION_MODES));
export const externalPermissionModeSchema = lazySchema(() =>
  z.enum(EXTERNAL_PERMISSION_MODES),
);

type ModeColorKey =
  | 'text'
  | 'planMode'
  | 'permission'
  | 'autoAccept'
  | 'error'
  | 'warning'
  | 'inactive';

type PermissionModeConfig = {
  title: string;
  shortTitle: string;
  symbol: string;
  color: ModeColorKey;
  external: ExternalPermissionMode;
};

const PERMISSION_MODE_CONFIG: Partial<
  Record<PermissionMode, PermissionModeConfig>
> = {
  default: {
    title: 'Manual',
    shortTitle: 'Manual',
    symbol: PAUSE_ICON,
    color: 'inactive',
    external: 'default',
  },
  plan: {
    title: 'Plan',
    shortTitle: 'Plan',
    symbol: PAUSE_ICON,
    color: 'planMode',
    external: 'plan',
  },
  acceptEdits: {
    title: 'Accept edits',
    shortTitle: 'Accept',
    symbol: '⏵⏵',
    color: 'autoAccept',
    external: 'acceptEdits',
  },
  bypassPermissions: {
    title: 'Bypass Permissions',
    shortTitle: 'Bypass',
    symbol: '⏵⏵',
    color: 'error',
    external: 'bypassPermissions',
  },
  dontAsk: {
    title: "Don't Ask",
    shortTitle: 'DontAsk',
    symbol: '⏵⏵',
    color: 'error',
    external: 'dontAsk',
  },
  ...(feature('TRANSCRIPT_CLASSIFIER')
    ? {
        auto: {
          title: 'Auto mode',
          shortTitle: 'Auto',
          symbol: '⏵⏵',
          color: 'warning' as ModeColorKey,
          external: 'default' as ExternalPermissionMode,
        },
      }
    : {}),
};

export function isExternalPermissionMode(
  mode: PermissionMode,
): mode is ExternalPermissionMode {
  if (process.env.USER_TYPE !== 'ant') {
    return true;
  }
  return mode !== 'auto' && mode !== 'bubble';
}

function getModeConfig(mode: PermissionMode): PermissionModeConfig {
  return PERMISSION_MODE_CONFIG[mode] ?? PERMISSION_MODE_CONFIG.default!;
}

export function toExternalPermissionMode(
  mode: PermissionMode,
): ExternalPermissionMode {
  return getModeConfig(mode).external;
}

export function permissionModeFromString(str: string): PermissionMode {
  return (PERMISSION_MODES as readonly string[]).includes(str)
    ? (str as PermissionMode)
    : 'default';
}

export function permissionModeTitle(mode: PermissionMode): string {
  return getModeConfig(mode).title;
}

export function isDefaultMode(_mode: PermissionMode | undefined): boolean {
  return false;
}

export function permissionModeShortTitle(mode: PermissionMode): string {
  return getModeConfig(mode).shortTitle;
}

export function permissionModeSymbol(mode: PermissionMode): string {
  return getModeConfig(mode).symbol;
}

export function getModeColor(mode: PermissionMode): ModeColorKey {
  return getModeConfig(mode).color;
}

