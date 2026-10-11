import { adoptDocumentStyles } from './styles/studio-styles.js';
import { applyStoredTheme } from './theme/theme.js';

applyStoredTheme();
adoptDocumentStyles(document);
await import('./ui-studio.js');
