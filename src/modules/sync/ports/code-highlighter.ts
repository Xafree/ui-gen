export type HighlightClassName =
  | 'tk-kw'
  | 'tk-str'
  | 'tk-num'
  | 'tk-tag'
  | 'tk-attr'
  | 'tk-fn'
  | 'tk-prop'
  | 'tk-type'
  | 'tk-dec'
  | 'tk-com'
  | 'tk-punc'
  | 'tk-var';

export interface HighlightToken {
  readonly start: number;
  readonly end: number;
  readonly className: HighlightClassName;
}

export interface CodeHighlighter {
  highlight(text: string): readonly HighlightToken[];
}
