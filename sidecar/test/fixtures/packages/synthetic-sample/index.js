import { createElement } from 'react';

export function Label({ children, ...rest }) {
  return createElement('button', rest, children);
}

export function Field({ children, ...rest }) {
  return createElement('input', rest, children);
}

export function Silent() {
  return createElement('span', null, 'fixed');
}
