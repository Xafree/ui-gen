export interface TextPosition {
  readonly line: number;
  readonly column: number;
}

function lineStartsOf(text: string): number[] {
  const starts = [0];
  let lineBreak = text.indexOf('\n');
  while (lineBreak !== -1) {
    starts.push(lineBreak + 1);
    lineBreak = text.indexOf('\n', lineBreak + 1);
  }
  return starts;
}

export function createPositionLocator(text: string): (offset: number) => TextPosition {
  const starts = lineStartsOf(text);
  return (offset) => {
    let low = 0;
    let high = starts.length - 1;
    while (low < high) {
      const middle = Math.ceil((low + high) / 2);
      if ((starts[middle] ?? 0) <= offset) {
        low = middle;
      } else {
        high = middle - 1;
      }
    }
    return { line: low + 1, column: offset - (starts[low] ?? 0) + 1 };
  };
}
