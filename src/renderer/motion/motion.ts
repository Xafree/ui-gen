import { gsap } from 'gsap';

export const DUR = { instant: 0.08, fast: 0.14, base: 0.2, slow: 0.28 } as const;
export const EASE = { out: 'power3.out', in: 'power2.in', inOut: 'power2.inOut', soft: 'power1.out' } as const;

export type MotionMode = 'full' | 'fade' | 'none';

export interface EnterOptions {
  readonly from?: number;
  readonly y?: number;
  readonly duration?: number;
  readonly stagger?: number;
}

const reducedMotion = { active: matchMedia('(prefers-reduced-motion: reduce)').matches };

gsap.defaults({ duration: DUR.base, ease: EASE.out, overwrite: 'auto' });
gsap
  .matchMedia()
  .add({ reduce: '(prefers-reduced-motion: reduce)', full: '(prefers-reduced-motion: no-preference)' }, (context) => {
    reducedMotion.active = context.conditions?.['reduce'] === true;
  });

let motionLayer: HTMLDivElement | null = null;

export function motionMode(): MotionMode {
  if (document.visibilityState !== 'visible') return 'none';
  return reducedMotion.active ? 'fade' : 'full';
}

export function safety<T extends gsap.core.Animation>(animation: T, extraMs = 350): T {
  window.setTimeout(() => {
    if (animation.isActive() || animation.progress() < 1) animation.progress(1);
  }, animation.totalDuration() * 1000 + extraMs);
  return animation;
}

export function enter(targets: readonly Element[], options: EnterOptions = {}): gsap.core.Tween | null {
  const mode = motionMode();
  if (targets.length === 0 || mode === 'none') return null;
  const opacity = options.from ?? 0.45;
  const from: gsap.TweenVars = mode === 'full' ? { opacity, y: options.y ?? 6 } : { opacity };
  const to: gsap.TweenVars =
    mode === 'full'
      ? {
          opacity: 1,
          y: 0,
          duration: options.duration ?? DUR.slow,
          ease: EASE.out,
          stagger: options.stagger ?? 0.03,
          clearProps: 'opacity,transform',
        }
      : { opacity: 1, duration: DUR.base, ease: EASE.out, stagger: 0, clearProps: 'opacity,transform' };
  return safety(gsap.fromTo([...targets], from, to));
}

export function reveal(targets: readonly Element[]): gsap.core.Tween | null {
  return enter(targets, { y: 4, from: 0.5, stagger: 0.02, duration: DUR.base });
}

export function openPanel(element: HTMLElement, origin = 'top left'): Promise<HTMLElement> {
  const wasHidden = element.hidden;
  element.hidden = false;
  const mode = motionMode();
  if (mode === 'none') return Promise.resolve(element);
  gsap.killTweensOf(element);
  if (wasHidden) {
    gsap.set(element, mode === 'full' ? { opacity: 0.001, y: -4, scale: 0.98, transformOrigin: origin } : { opacity: 0.001 });
  }
  return new Promise((resolve) => {
    safety(
      gsap.to(element, {
        opacity: 1,
        y: 0,
        scale: 1,
        duration: mode === 'full' ? DUR.base : DUR.fast,
        ease: EASE.out,
        clearProps: 'opacity,transform,transformOrigin',
        onComplete: () => {
          resolve(element);
        },
      }),
    );
  });
}

export function closePanel(element: HTMLElement): Promise<HTMLElement> {
  if (element.hidden) return Promise.resolve(element);
  const mode = motionMode();
  if (mode === 'none') {
    element.hidden = true;
    return Promise.resolve(element);
  }
  gsap.killTweensOf(element);
  return new Promise((resolve) => {
    safety(
      gsap.to(element, {
        opacity: 0,
        y: mode === 'full' ? -4 : 0,
        scale: mode === 'full' ? 0.98 : 1,
        duration: DUR.fast,
        ease: EASE.in,
        onComplete: () => {
          element.hidden = true;
          gsap.set(element, { clearProps: 'opacity,transform,transformOrigin' });
          resolve(element);
        },
      }),
    );
  });
}

function guardModal(dialog: HTMLDialogElement, scale: number): void {
  dialog.style.transform = `scale(${String(scale)})`;
}

function originFrom(trigger: Element | null, dialog: HTMLDialogElement): string {
  if (trigger === null) return '50% 50%';
  const source = trigger.getBoundingClientRect();
  const target = dialog.getBoundingClientRect();
  const x = Math.round(source.left + source.width / 2 - target.left);
  const y = Math.round(source.top + source.height / 2 - target.top);
  return `${String(x)}px ${String(y)}px`;
}

export function dialogOpen(dialog: HTMLDialogElement, trigger: Element | null): Promise<HTMLDialogElement> {
  if (!dialog.open) dialog.showModal();
  const mode = motionMode();
  if (mode === 'none') return Promise.resolve(dialog);
  gsap.killTweensOf(dialog);
  const origin = originFrom(trigger, dialog);
  guardModal(dialog, mode === 'full' ? 0.97 : 0.999);
  const from: gsap.TweenVars =
    mode === 'full' ? { opacity: 0.001, scale: 0.97, transformOrigin: origin } : { opacity: 0.001 };
  return new Promise((resolve) => {
    safety(
      gsap.fromTo(dialog, from, {
        opacity: 1,
        scale: 1,
        duration: mode === 'full' ? DUR.slow : DUR.fast,
        ease: EASE.out,
        clearProps: 'opacity',
        onComplete: () => {
          resolve(dialog);
        },
      }),
    );
  });
}

export function dialogClose(dialog: HTMLDialogElement, returnValue?: string): Promise<HTMLDialogElement> {
  if (!dialog.open) return Promise.resolve(dialog);
  const finish = (): void => {
    dialog.close(returnValue);
    gsap.set(dialog, { clearProps: 'opacity,transform,transformOrigin' });
  };
  const mode = motionMode();
  if (mode === 'none') {
    finish();
    return Promise.resolve(dialog);
  }
  gsap.killTweensOf(dialog);
  guardModal(dialog, 0.999);
  return new Promise((resolve) => {
    safety(
      gsap.to(dialog, {
        opacity: 0,
        scale: mode === 'full' ? 0.98 : 1,
        duration: DUR.fast,
        ease: EASE.in,
        onComplete: () => {
          finish();
          resolve(dialog);
        },
      }),
    );
  });
}

function layer(): HTMLDivElement {
  if (motionLayer?.isConnected === true) return motionLayer;
  const created = document.createElement('div');
  created.className = 'ui-motion-layer';
  created.setAttribute('aria-hidden', 'true');
  document.body.append(created);
  motionLayer = created;
  return created;
}

export function highlight(targets: readonly Element[]): gsap.core.Timeline | null {
  const mode = motionMode();
  if (targets.length === 0 || mode === 'none') return null;
  const host = layer();
  const timeline = gsap.timeline({ defaults: { ease: EASE.out } });
  const created: HTMLElement[] = [];
  targets.forEach((target, index) => {
    const rect = target.getBoundingClientRect();
    if (rect.width === 0 && rect.height === 0) return;
    const at = index * 0.05;
    const flash = document.createElement('div');
    flash.className = 'ui-sync-flash';
    flash.style.cssText = `left:${String(rect.left)}px;top:${String(rect.top)}px;width:${String(rect.width)}px;height:${String(rect.height)}px;opacity:0`;
    host.append(flash);
    created.push(flash);
    timeline
      .to(flash, { opacity: 1, duration: DUR.fast }, at)
      .to(flash, { opacity: 0, duration: DUR.slow, ease: EASE.in }, at + DUR.fast + 0.22);
  });
  const cleanup = (): void => {
    for (const node of created) node.remove();
  };
  timeline.eventCallback('onComplete', cleanup);
  window.setTimeout(cleanup, timeline.totalDuration() * 1000 + 600);
  return safety(timeline);
}

export function swap(container: HTMLElement, update: () => void): Promise<HTMLElement> {
  const mode = motionMode();
  if (mode === 'none') {
    update();
    return Promise.resolve(container);
  }
  gsap.killTweensOf(container);
  return new Promise((resolve) => {
    const timeline = gsap.timeline({
      onComplete: () => {
        gsap.set(container, { clearProps: 'opacity,transform' });
        resolve(container);
      },
    });
    timeline
      .to(container, { opacity: 0.35, duration: 0.09, ease: EASE.in })
      .add(update)
      .fromTo(container, { y: mode === 'full' ? 3 : 0 }, { opacity: 1, y: 0, duration: DUR.base, ease: EASE.out });
    safety(timeline);
  });
}

const INDICATOR_BASE_WIDTH = 100;
const placedIndicators = new WeakSet<Element>();

function moveIndicator(list: HTMLElement, indicator: HTMLElement, instant: boolean): void {
  const selected = list.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]');
  if (selected === null) return;
  const target = { x: selected.offsetLeft, scaleX: selected.offsetWidth / INDICATOR_BASE_WIDTH };
  const immediate = instant || !placedIndicators.has(indicator) || motionMode() !== 'full';
  placedIndicators.add(indicator);
  if (immediate) gsap.set(indicator, target);
  else safety(gsap.to(indicator, { ...target, duration: DUR.base, ease: EASE.out }));
}

function createIndicator(list: HTMLElement): HTMLElement {
  const indicator = document.createElement('span');
  indicator.className = 'ui-tabs__indicator';
  indicator.setAttribute('aria-hidden', 'true');
  list.append(indicator);
  list.classList.add('has-indicator');
  new ResizeObserver(() => { moveIndicator(list, indicator, true); }).observe(list);
  void document.fonts.ready.then(() => { moveIndicator(list, indicator, true); });
  return indicator;
}

export function placeTabIndicator(list: HTMLElement): void {
  moveIndicator(list, list.querySelector<HTMLElement>('.ui-tabs__indicator') ?? createIndicator(list), false);
}
