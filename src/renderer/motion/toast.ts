import { gsap } from 'gsap';
import { render } from 'lit';
import type { IconName } from '../icons/icon-paths.js';
import { icon } from '../icons/icon.js';
import { DUR, EASE, motionMode, safety } from './motion.js';

export type ToastTone = 'info' | 'success' | 'warning' | 'danger';

export interface ToastAction {
  readonly label: string;
  readonly onClick: () => void;
}

export interface ToastRequest {
  readonly tone: ToastTone;
  readonly title: string;
  readonly message: string;
  readonly action?: ToastAction;
  readonly duration?: number;
}

export interface ToastHandle {
  readonly element: HTMLLIElement;
  dismiss(): Promise<void>;
}

const TONE_ICON: Readonly<Record<ToastTone, IconName>> = {
  info: 'info',
  success: 'succes',
  warning: 'alerte',
  danger: 'erreur',
};
const MAX_TOASTS = 3;
const DEFAULT_DURATION = 5000;
const MIN_REMAINING = 800;

function paragraph(className: string, text: string): HTMLParagraphElement {
  const element = document.createElement('p');
  element.className = className;
  element.textContent = text;
  return element;
}

function toastBody(request: ToastRequest, dismiss: () => void): HTMLDivElement {
  const body = document.createElement('div');
  body.append(paragraph('ui-toast__title', request.title), paragraph('ui-toast__text', request.message));
  const { action } = request;
  if (action !== undefined) {
    const actions = document.createElement('div');
    actions.className = 'ui-toast__actions';
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'ui-btn ui-btn--secondary ui-btn--sm';
    button.textContent = action.label;
    button.addEventListener('click', () => {
      action.onClick();
      dismiss();
    });
    actions.append(button);
    body.append(actions);
  }
  return body;
}

function enterToast(item: HTMLLIElement): void {
  const mode = motionMode();
  if (mode === 'full') {
    safety(gsap.fromTo(item, { opacity: 0.001, y: 8 }, { opacity: 1, y: 0, duration: DUR.slow, ease: EASE.out, clearProps: 'opacity,transform' }));
  } else if (mode === 'fade') {
    safety(gsap.fromTo(item, { opacity: 0.001 }, { opacity: 1, duration: DUR.fast, clearProps: 'opacity' }));
  }
}

function leaveToast(item: HTMLLIElement): Promise<void> {
  const mode = motionMode();
  if (mode === 'none') {
    item.remove();
    return Promise.resolve();
  }
  return new Promise((resolve) => {
    safety(
      gsap.to(item, {
        opacity: 0,
        y: mode === 'full' ? 8 : 0,
        duration: DUR.fast,
        ease: EASE.in,
        onComplete: () => {
          item.remove();
          resolve();
        },
      }),
    );
  });
}

export function showToast(region: HTMLOListElement, request: ToastRequest): ToastHandle {
  const item = document.createElement('li');
  item.className = `ui-toast ui-toast--${request.tone}`;
  if (request.tone === 'danger') item.setAttribute('role', 'alert');
  let remaining = request.duration ?? (request.tone === 'danger' ? 0 : DEFAULT_DURATION);
  let timer: number | null = null;
  let startedAt = 0;
  let leaving: Promise<void> | null = null;
  const dismiss = (): Promise<void> => {
    if (timer !== null) window.clearTimeout(timer);
    leaving ??= leaveToast(item);
    return leaving;
  };
  const startTimer = (): void => {
    if (remaining === 0 || leaving !== null) return;
    startedAt = Date.now();
    timer = window.setTimeout(() => void dismiss(), remaining);
  };
  const pauseTimer = (): void => {
    if (timer === null) return;
    window.clearTimeout(timer);
    timer = null;
    remaining = Math.max(MIN_REMAINING, remaining - (Date.now() - startedAt));
  };
  const iconHolder = document.createElement('span');
  iconHolder.className = 'ui-toast__icon';
  render(icon(TONE_ICON[request.tone]), iconHolder);
  const close = document.createElement('button');
  close.type = 'button';
  close.className = 'ui-icon-btn';
  close.setAttribute('aria-label', 'Fermer la notification');
  render(icon('fermer'), close);
  close.addEventListener('click', () => void dismiss());
  item.append(iconHolder, toastBody(request, () => void dismiss()), close);
  region.append(item);
  while (region.children.length > MAX_TOASTS) region.firstElementChild?.remove();
  enterToast(item);
  item.addEventListener('mouseenter', pauseTimer);
  item.addEventListener('focusin', pauseTimer);
  item.addEventListener('mouseleave', startTimer);
  item.addEventListener('focusout', startTimer);
  startTimer();
  return { element: item, dismiss };
}
