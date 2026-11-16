export type BrainToolState = 'pending' | 'running' | 'succeeded' | 'failed' | 'cancelled' | 'timed_out' | 'interrupted';

/** Bounded, frontend-neutral values. Neither components nor input execute tools. */
export type BrainToolPresentation = Readonly<{
  kind: 'tool';
  id: string;
  label: string;
  status: BrainToolState;
  action?: string;
  summary?: string;
  detail?: string;
  expanded?: boolean;
}>;

export type PermissionPresentation = Readonly<{
  requestId: string;
  label: string;
  summary: string;
}>;

export type ContextSummary = Readonly<{ retrievalCount: number; contextTokenCount: number }>;
