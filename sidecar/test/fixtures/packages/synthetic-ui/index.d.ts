export interface ButtonProps {
  variant: 'solid' | 'outline';
  size: 'sm' | 'md';
  disabled?: boolean;
}

export declare function Button(props: ButtonProps): unknown;

export declare function formatLabel(text: string): string;
