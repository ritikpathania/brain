/**
 * Large-paste handling (reference inputPaste contract, Brain-branded copy):
 * ≥ TRUNCATION_THRESHOLD chars collapse to a short counted placeholder; the
 * full text rides along in pastedContents and expands back at submit time.
 */

export const TRUNCATION_THRESHOLD = 10_000;

export interface StoredPaste {
  id: string;
  content: string;
}

export function placeholderFor(idNumber: number, content: string): string {
  const lines = content.split('\n').length;
  return `[Pasted text #${idNumber} +${lines} lines]`;
}

export function processPaste(
  text: string,
  pasteCounter: number,
): { inserted: string; stored?: StoredPaste; nextCounter: number } {
  if (text.length < TRUNCATION_THRESHOLD) {
    return { inserted: text, nextCounter: pasteCounter };
  }
  const idNumber = pasteCounter + 1;
  return {
    inserted: placeholderFor(idNumber, text),
    stored: { id: `paste_${idNumber}`, content: text },
    nextCounter: idNumber,
  };
}

const PLACEHOLDER_RE = /\[Pasted text #(\d+) \+\d+ lines\]/g;

export function expandPastedPlaceholders(
  value: string,
  pastedContents: Record<string, string>,
): string {
  return value.replace(PLACEHOLDER_RE, (match, num: string) => {
    const stored = pastedContents[`paste_${num}`];
    return stored !== undefined ? stored : match;
  });
}
