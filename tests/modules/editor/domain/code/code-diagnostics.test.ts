import { describe, expect, it } from 'vitest';
import {
  displayedDiagnostics,
  markLines,
  type Diagnostic,
} from '../../../../../src/modules/editor/domain/code/code-diagnostics.js';
import { splitCodeLines } from '../../../../../src/modules/editor/domain/code/code-lines.js';
import type { OpenedComponent } from '../../../../../src/modules/sync/application/components/component-views.js';

const TEXT = 'const a = 1;\nconst b = ${x};\nconst c = 3';

function diagnostic(partial: Partial<Diagnostic> & Pick<Diagnostic, 'range' | 'line'>): Diagnostic {
  return { column: 1, severity: 'warning', code: 'ui-gen(visual-sync)', message: 'message', ...partial };
}

function marksFor(diagnostics: readonly Diagnostic[], highlight = [] as Parameters<typeof splitCodeLines>[1]) {
  return markLines(splitCodeLines(TEXT, highlight), diagnostics);
}

describe('markLines', () => {
  it('marks nothing without diagnostics', () => {
    expect(marksFor([])).toEqual([null, null, null]);
  });

  it('underlines exactly the diagnostic range and keeps the rest of the line apart', () => {
    const start = TEXT.indexOf('${x}');
    const marks = marksFor([diagnostic({ range: { start, end: start + 4 }, line: 2 })]);

    expect(marks[0]).toBeNull();
    expect(marks[1]?.fragments).toEqual([
      { diagnostic: null, segments: [{ text: 'const b = ', className: null }] },
      { diagnostic: 0, segments: [{ text: '${x}', className: null }] },
      { diagnostic: null, segments: [{ text: ';', className: null }] },
    ]);
    expect(marks[1]?.severity).toBe('warning');
    expect(marks[1]?.message).toBe('message');
  });

  it('keeps the highlight classes inside and around the underlined range', () => {
    const start = TEXT.indexOf('b = ${x}');
    const marks = marksFor([diagnostic({ range: { start: start + 4, end: start + 8 }, line: 2 })], [
      { start: start - 6, end: start + 6, className: 'tk-kw' },
    ]);

    expect(marks[1]?.fragments.map(({ diagnostic: index, segments }) => [index, segments.map(({ text, className }) => [text, className])])).toEqual([
      [null, [['const b = ', 'tk-kw']]],
      [0, [['${', 'tk-kw'], ['x}', null]]],
      [null, [[';', null]]],
    ]);
  });

  it('underlines the last character when the range sits at the end of the line', () => {
    const end = TEXT.indexOf('\n');
    const marks = marksFor([diagnostic({ range: { start: end, end: end + 1 }, line: 1, severity: 'error' })]);

    expect(marks[0]?.severity).toBe('error');
    expect(marks[0]?.fragments).toEqual([
      { diagnostic: null, segments: [{ text: 'const a = 1', className: null }] },
      { diagnostic: 0, segments: [{ text: ';', className: null }] },
    ]);
    expect(marks[1]).toBeNull();
  });

  it('widens an empty range in the middle of a line to one character', () => {
    const marks = marksFor([diagnostic({ range: { start: 6, end: 6 }, line: 1 })]);

    expect(marks[0]?.fragments.map(({ segments }) => segments.map(({ text }) => text).join(''))).toEqual(['const ', 'a', ' = 1;']);
  });

  it('keeps a focusable empty mark on an empty line', () => {
    const lines = splitCodeLines('a\n\nb', []);
    const marks = markLines(lines, [diagnostic({ range: { start: 2, end: 3 }, line: 2 })]);

    expect(marks[1]?.fragments).toEqual([{ diagnostic: 0, segments: [] }]);
  });

  it('clips a range spanning several lines to the line the diagnostic is anchored on', () => {
    const marks = marksFor([diagnostic({ range: { start: 6, end: 30 }, line: 1 })]);

    expect(marks[0]?.fragments.map(({ segments }) => segments.map(({ text }) => text).join(''))).toEqual(['const ', 'a = 1;']);
    expect(marks[1]).toBeNull();
  });

  it('lets an error lead the line over a warning and numbers the diagnostics in order', () => {
    const marks = marksFor([
      diagnostic({ range: { start: 0, end: 5 }, line: 1, message: 'first' }),
      diagnostic({ range: { start: 6, end: 7 }, line: 1, severity: 'error', message: 'second' }),
    ]);

    expect(marks[0]?.severity).toBe('error');
    expect(marks[0]?.message).toBe('second');
    expect(marks[0]?.fragments.map(({ diagnostic: index }) => index)).toEqual([0, null, 1, null]);
  });

  it('ignores a range that starts inside the previous one', () => {
    const marks = marksFor([
      diagnostic({ range: { start: 0, end: 8 }, line: 1 }),
      diagnostic({ range: { start: 4, end: 10 }, line: 1 }),
    ]);

    expect(marks[0]?.fragments.map(({ diagnostic: index }) => index)).toEqual([0, null]);
  });
});

describe('displayedDiagnostics', () => {
  const model = (diagnostics: readonly Diagnostic[]): OpenedComponent['model'] => ({
    tag: 'x-a',
    className: 'XA',
    version: 'v1',
    template: [],
    properties: [],
    variantProperty: null,
    variants: [],
    styles: [],
    diagnostics,
  });
  const early = diagnostic({ range: { start: 1, end: 2 }, line: 1, message: 'early' });
  const late = diagnostic({ range: { start: 5, end: 6 }, line: 2, message: 'late' });

  it('shows nothing when no component is open', () => {
    expect(displayedDiagnostics(null)).toEqual([]);
  });

  it('shows the diagnostics of the model of an opened component, in the order of the code', () => {
    const opened: OpenedComponent = { path: 'a.ts', text: '', model: model([late, early]), highlight: [] };

    expect(displayedDiagnostics(opened)).toEqual([early, late]);
  });

  it('shows the diagnostics of the change rather than those of the model it keeps', () => {
    const change = { path: 'a.ts', text: '', model: model([early]), highlight: [], diagnostics: [late] };

    expect(displayedDiagnostics(change)).toEqual([late]);
  });

  it('shows nothing for a change without diagnostics even when the model has some', () => {
    const change = { path: 'a.ts', text: '', model: model([early]), highlight: [], diagnostics: [] };

    expect(displayedDiagnostics(change)).toEqual([]);
  });
});
