export type ClickHandler = (event: unknown) => void;

export interface BaseAttributes {
  id?: string;
  role?: string;
  hidden?: boolean;
  tabIndex?: number;
  align?: 'start' | 'end';
  className?: string;
  children?: unknown;
  onClick?: ClickHandler;
}
