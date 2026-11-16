export type JsonRecord = Record<string, unknown>;

export function isRecord(value: unknown): value is JsonRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function requiredString(
  value: JsonRecord,
  field: string,
  error: (message: string) => Error,
): string {
  const candidate = value[field];
  if (typeof candidate !== 'string' || candidate.length === 0) {
    throw error(`missing non-empty string field '${field}'`);
  }
  return candidate;
}

export function requiredNonNegativeInteger(
  value: JsonRecord,
  field: string,
  error: (message: string) => Error,
): number {
  const candidate = value[field];
  if (!Number.isSafeInteger(candidate) || (candidate as number) < 0) {
    throw error(`missing non-negative integer field '${field}'`);
  }
  return candidate as number;
}

export function requiredStringArray(
  value: JsonRecord,
  field: string,
  error: (message: string) => Error,
): readonly string[] {
  const candidate = value[field];
  if (!Array.isArray(candidate) || candidate.some(item => typeof item !== 'string')) {
    throw error(`missing string-array field '${field}'`);
  }
  return Object.freeze([...candidate] as string[]);
}

export function parseJsonObject(
  value: unknown,
  error: (message: string) => Error,
): JsonRecord {
  if (isRecord(value)) return value;
  if (typeof value !== 'string') throw error('expected a JSON object');
  try {
    const parsed: unknown = JSON.parse(value);
    if (!isRecord(parsed)) throw error('expected a JSON object');
    return parsed;
  } catch (cause) {
    if (cause instanceof Error && cause.message.includes('expected a JSON object')) throw cause;
    throw error('body is not valid JSON');
  }
}
