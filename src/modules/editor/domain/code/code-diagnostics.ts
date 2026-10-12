import type { ChangedComponent, DiagnosedComponent, Diagnostic } from '../model/edited-component.js';
import type { CodeLine, CodeSegment } from './code-lines.js';

export type { Diagnostic };
export type DiagnosticSeverity = Diagnostic['severity'];

export interface LineFragment {
  readonly diagnostic: number | null;
  readonly segments: readonly CodeSegment[];
}

export interface LineDiagnostics {
  readonly severity: DiagnosticSeverity;
  readonly message: string;
  readonly fragments: readonly LineFragment[];
}

interface Span {
  readonly start: number;
  readonly end: number;
}

interface IndexedDiagnostic {
  readonly index: number;
  readonly diagnostic: Diagnostic;
}

function isChange(component: DiagnosedComponent): component is ChangedComponent {
  return 'diagnostics' in component;
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}

export function displayedDiagnostics(component: DiagnosedComponent | null): Diagnostic[] {
  if (component === null) return [];
  const diagnostics = isChange(component) ? component.diagnostics : component.model.diagnostics;
  return [...diagnostics].sort((a, b) => a.range.start - b.range.start);
}

function visibleSpan(range: Span, line: CodeLine): Span {
  const start = clamp(range.start, line.start, line.end);
  const end = clamp(range.end, start, line.end);
  if (end > start || line.end === line.start) return { start, end };
  return start < line.end ? { start, end: start + 1 } : { start: start - 1, end: start };
}

function segmentsBetween(line: CodeLine, from: number, to: number): CodeSegment[] {
  const segments: CodeSegment[] = [];
  let position = line.start;
  for (const segment of line.segments) {
    const start = Math.max(from, position);
    const end = Math.min(to, position + segment.text.length);
    if (end > start) segments.push({ text: segment.text.slice(start - position, end - position), className: segment.className });
    position += segment.text.length;
  }
  return segments;
}

function fragmentsOf(line: CodeLine, spans: readonly (Span & { readonly index: number })[]): LineFragment[] {
  const fragments: LineFragment[] = [];
  let cursor = line.start;
  const pushPlain = (to: number): void => {
    const segments = segmentsBetween(line, cursor, to);
    if (segments.length > 0) fragments.push({ diagnostic: null, segments });
  };
  for (const span of spans) {
    if (span.start < cursor) continue;
    pushPlain(span.start);
    fragments.push({ diagnostic: span.index, segments: segmentsBetween(line, span.start, span.end) });
    cursor = span.end;
  }
  pushPlain(line.end);
  return fragments;
}

function marksOf(line: CodeLine, entries: readonly IndexedDiagnostic[]): LineDiagnostics {
  const lead = entries.find(({ diagnostic }) => diagnostic.severity === 'error') ?? entries[0];
  const spans = entries.map(({ index, diagnostic }) => ({ index, ...visibleSpan(diagnostic.range, line) }));
  return {
    severity: lead?.diagnostic.severity ?? 'warning',
    message: lead?.diagnostic.message ?? '',
    fragments: fragmentsOf(line, spans),
  };
}

export function markLines(lines: readonly CodeLine[], diagnostics: readonly Diagnostic[]): (LineDiagnostics | null)[] {
  const byLine = new Map<number, IndexedDiagnostic[]>();
  diagnostics.forEach((diagnostic, index) => {
    byLine.set(diagnostic.line, [...(byLine.get(diagnostic.line) ?? []), { index, diagnostic }]);
  });
  return lines.map((line) => {
    const entries = byLine.get(line.number);
    return entries === undefined ? null : marksOf(line, entries);
  });
}
