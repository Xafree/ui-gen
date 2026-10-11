import { html, type TemplateResult } from 'lit';
import { unsafeSVG } from 'lit/directives/unsafe-svg.js';
import { ICON_PATHS, type IconName } from './icon-paths.js';

export function icon(name: IconName, className = ''): TemplateResult {
  const classes = className === '' ? 'icon' : `icon ${className}`;
  return html`<svg class=${classes} aria-hidden="true" focusable="false" viewBox="0 0 256 256">${unsafeSVG(ICON_PATHS[name])}</svg>`;
}

export function studioMark(): TemplateResult {
  return html`<svg viewBox="0 0 16 16" aria-hidden="true" focusable="false"><path fill="currentColor" d="M4.25 10.5a3.75 3.75 0 0 1 7.5 0Z"/><rect x="2.5" y="11.75" width="11" height="1.25" rx=".6" fill="currentColor"/></svg>`;
}
