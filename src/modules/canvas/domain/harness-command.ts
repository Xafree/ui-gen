export type HarnessCommand =
  | { readonly type: 'hit-test'; readonly x: number; readonly y: number }
  | { readonly type: 'dark-preview'; readonly enabled: boolean };
