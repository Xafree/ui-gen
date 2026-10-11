import { css, unsafeCSS, type CSSResult } from 'lit';
import fontsCss from './fonts.css?inline';
import studioCss from './studio.css?inline';
import tokensCss from './tokens.css?inline';

export const fontStyles: CSSResult = unsafeCSS(fontsCss);
export const tokensStyles: CSSResult = unsafeCSS(tokensCss);
export const studioStyles: CSSResult = unsafeCSS(studioCss);
export const hostContents: CSSResult = css`
  :host {
    display: contents;
  }
`;

function sheetOf(style: CSSResult): CSSStyleSheet {
  const sheet = style.styleSheet;
  if (sheet === undefined) throw new Error('Constructable stylesheets are unavailable');
  return sheet;
}

export function adoptDocumentStyles(target: Document): void {
  target.adoptedStyleSheets = [
    ...target.adoptedStyleSheets,
    sheetOf(fontStyles),
    sheetOf(tokensStyles),
    sheetOf(studioStyles),
  ];
}
