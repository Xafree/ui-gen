export interface Segment {
  readonly text: string;
  readonly start: number;
}

const OPENERS = '([';
const CLOSERS = ')]';

export function isWhitespace(character: string): boolean {
  return character.trim() === '';
}

export function splitTopLevel(text: string, isSeparator: (character: string) => boolean): Segment[] {
  const segments: Segment[] = [];
  let depth = 0;
  let start = 0;
  const closeSegment = (end: number): void => {
    if (end > start) {
      segments.push({ text: text.slice(start, end), start });
    }
  };
  for (let index = 0; index < text.length; index += 1) {
    const character = text.charAt(index);
    if (depth === 0 && isSeparator(character)) {
      closeSegment(index);
      start = index + 1;
    } else if (OPENERS.includes(character)) {
      depth += 1;
    } else if (CLOSERS.includes(character)) {
      depth = Math.max(0, depth - 1);
    }
  }
  closeSegment(text.length);
  return segments;
}
