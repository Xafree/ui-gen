import type { BaseAttributes, ClickHandler } from 'synthetic-base-types';

export interface CardProps extends BaseAttributes, Pick<HTMLElement, 'title' | 'lang'> {
  tone: 'quiet' | 'loud' | 'neutral';
  count?: number;
  heading: string;
  dense?: boolean;
  meta?: { author: string };
  children?: unknown;
  className?: string;
  style?: Record<string, string>;
  onClick?: ClickHandler;
  onSelect?: (id: string) => void;
  renderIcon?: () => unknown;
}

export declare function Card(props: CardProps): unknown;

export interface PanelProps extends BaseAttributes {
  side: 'left' | 'right';
  open?: boolean;
}

export declare class Panel {
  constructor(props: PanelProps);
}
