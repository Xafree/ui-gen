import { PLAN_BADGE, positionOf, readFixture, replaceOnce, sliceOf, type Model } from './sync-fixtures';

export type StyleRuleShape = Model['styles'][number];
export type VisualValueShape = StyleRuleShape['values'][number];

export interface ValueSummary {
  readonly key: VisualValueShape['key'];
  readonly raw: string;
  readonly token: string | null;
}

export const NOT_PROJECTABLE_RULE_MESSAGE =
  "Cette règle ne peut pas être affichée dans l'éditeur visuel. Elle est conservée dans le code.";

export const BADGE_HOST_RULE = ':host { display: inline-flex; }';

export function badgeWithStyles(css: string): string {
  return replaceOnce(readFixture(PLAN_BADGE), BADGE_HOST_RULE, css);
}

export function rulesWithSelector(model: Model, selector: string): StyleRuleShape[] {
  return model.styles.filter((rule) => rule.selector === selector);
}

export function findRule(model: Model, selector: string): StyleRuleShape {
  const found = rulesWithSelector(model, selector);
  const [rule] = found;
  if (rule === undefined || found.length !== 1) {
    throw new Error(`expected exactly one rule ${selector}, found ${String(found.length)}`);
  }
  return rule;
}

function compareSummaries(left: ValueSummary, right: ValueSummary): number {
  return left.key.localeCompare(right.key) || left.raw.localeCompare(right.raw);
}

export function sortedSummaries(values: readonly ValueSummary[]): ValueSummary[] {
  return [...values].sort(compareSummaries);
}

export function summariesOf(rule: StyleRuleShape): ValueSummary[] {
  return sortedSummaries(rule.values.map(({ key, raw, token }) => ({ key, raw, token })));
}

export function ruleWarnings(model: Model, text: string) {
  return model.diagnostics
    .filter((diagnostic) => diagnostic.message === NOT_PROJECTABLE_RULE_MESSAGE)
    .map((diagnostic) => ({
      severity: diagnostic.severity,
      code: diagnostic.code,
      line: diagnostic.line,
      column: diagnostic.column,
      expectedPosition: positionOf(text, diagnostic.range.start),
      source: sliceOf(text, diagnostic.range),
    }));
}
