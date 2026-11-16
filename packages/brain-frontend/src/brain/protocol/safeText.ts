import { stripVTControlCharacters } from 'node:util';

/** Never carry terminal instructions or an unbounded tool payload into state. */
export function safeToolText(value: string, limit = 4_096, maxLines = 20): string {
  const plain = stripVTControlCharacters(value)
    .replace(/\u001b\][^\u0007]*(?:\u0007|$)/g, '')
    .replace(/[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/g, '');
  const lines = plain.split('\n');
  if (plain.length <= limit && lines.length <= maxLines) return plain;
  const marker = maxLines > 1 ? '\n[output truncated]' : ' [truncated]';
  const bounded = lines.slice(0, maxLines > 1 ? maxLines - 1 : 1).join('\n').slice(0, Math.max(0, limit - marker.length));
  return bounded + marker;
}

export function toolAction(name: string, input: Readonly<Record<string, unknown>>): string {
  // Allowlisted operation text, not a dump of arbitrary tool arguments/credentials.
  return name === 'bash' && typeof input.command === 'string'
    ? safeToolText(input.command, 240, 1)
    : 'Requested Brain operation';
}
