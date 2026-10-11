import type { CodeHighlighter } from '../../ports/code-highlighter.js';
import { createTokenSink, type HighlightContext } from './highlight/highlight-context.js';
import { highlightTypeScript } from './highlight/highlight-typescript.js';

export function createTypeScriptHighlighter(): CodeHighlighter {
  return {
    highlight(text) {
      const sink = createTokenSink(text);
      const context: HighlightContext = {
        text,
        sink,
        highlightExpression: (start, end) => {
          highlightTypeScript(context, start, end);
        },
      };
      highlightTypeScript(context, 0, text.length);
      return sink.tokens();
    },
  };
}
