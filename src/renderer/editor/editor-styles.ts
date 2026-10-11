import { unsafeCSS, type CSSResult } from 'lit';
import { studioStyles } from '../styles/studio-styles.js';
import editorCss from './editor.css?inline';

export const editorStyles: CSSResult[] = [studioStyles, unsafeCSS(editorCss)];
