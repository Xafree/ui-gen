import { createElement } from 'react';

const STYLE = `
.sb { font: 14px/20px sans-serif; border: 1px solid #1976d2; border-radius: 4px; cursor: pointer; }
.sb--solid { background: #1976d2; color: #ffffff; }
.sb--solid:hover { background: #1565c0; }
.sb--outline { background: #ffffff; color: #1976d2; }
.sb--outline:hover { background: #e3f2fd; }
.sb--sm { padding: 2px 8px; }
.sb--md { padding: 6px 16px; }
.sb:disabled { opacity: 0.5; cursor: default; }
`;

export function Button({ variant = 'solid', size = 'md', disabled = false, label = 'Button', ...rest }) {
  return createElement(
    'button',
    { ...rest, className: `sb sb--${variant} sb--${size}`, disabled },
    createElement('style', null, STYLE),
    label,
  );
}

export function formatLabel(text) {
  return text.trim();
}
