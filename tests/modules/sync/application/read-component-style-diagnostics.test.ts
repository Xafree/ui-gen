import { describe, expect, it } from 'vitest';
import { BADGE_HOST_RULE, NOT_PROJECTABLE_RULE_MESSAGE, badgeWithStyles, findRule, ruleWarnings } from '../style-fixtures';
import { NOT_REPRESENTABLE_MESSAGE, read, replaceOnce, sliceOf } from '../sync-fixtures';

function expectSingleRuleWarning(text: string, expectedSource: (source: string) => void) {
  const model = read(text);
  const warnings = ruleWarnings(model, text);

  expect(model.diagnostics).toHaveLength(1);
  expect(warnings).toHaveLength(1);
  for (const warning of warnings) {
    expect(warning.severity).toBe('warning');
    expect(warning.code).toBe('ui-gen(visual-sync)');
    expect({ line: warning.line, column: warning.column }).toEqual(warning.expectedPosition);
    expectedSource(warning.source);
  }
  return model;
}

describe('style rules that the visual editor cannot show', () => {
  // covers: FCT-0039
  it('warns on a rule whose value is an unsafeCSS interpolation and projects no value from it', () => {
    const text = badgeWithStyles('.badge { color: ${unsafeCSS(tone)}; }');
    const model = expectSingleRuleWarning(text, (source) => {
      expect(source).toContain('${unsafeCSS(tone)}');
    });

    const projected = model.styles.flatMap((rule) => rule.values.map((value) => sliceOf(text, value.range)));
    expect(projected.filter((source) => source.includes('unsafeCSS'))).toEqual([]);
    expect(model.diagnostics[0]?.line).toBe(8);
  });

  // covers: FCT-0039
  it('warns on a rule whose selector is an unsafeCSS interpolation', () => {
    const text = badgeWithStyles('${unsafeCSS(selector)} { color: red; }');

    expectSingleRuleWarning(text, (source) => {
      expect(source).toContain('${unsafeCSS(selector)}');
    });
  });

  // covers: FCT-0039
  it('warns once on a media query and projects none of the rules it holds', () => {
    const media = '@media (min-width: 600px) {\n\t\t\t.badge { font-size: 18px; }\n\t\t}';
    const text = badgeWithStyles(`.badge { color: red; }\n\t\t${media}`);
    const model = expectSingleRuleWarning(text, (source) => {
      expect(source.startsWith('@media (min-width: 600px)')).toBe(true);
      expect(source).toContain('.badge { font-size: 18px; }');
    });

    expect(model.styles.filter((rule) => rule.selector === '.badge').map((rule) => rule.values.map(({ raw }) => raw))).toEqual([
      ['red'],
    ]);
  });

  // covers: FCT-0039
  it('warns at the line and column where the rule starts', () => {
    const text = badgeWithStyles('.badge { color: red; }\n\t\t@media print {\n\t\t\tstrong { color: black; }\n\t\t}');
    const model = read(text);

    expect(ruleWarnings(model, text).map(({ line, column }) => ({ line, column }))).toEqual([{ line: 9, column: 3 }]);
  });

  // covers: FCT-0039
  it('counts rule warnings alongside the template warnings', () => {
    const text = replaceOnce(badgeWithStyles('.badge { color: ${unsafeCSS(tone)}; }'), '${this.label}', '${this.title}');
    const model = read(text);

    expect(model.diagnostics).toHaveLength(2);
    expect(model.diagnostics.map((diagnostic) => diagnostic.message).toSorted((left, right) => left.localeCompare(right))).toEqual(
      [NOT_PROJECTABLE_RULE_MESSAGE, NOT_REPRESENTABLE_MESSAGE].toSorted((left, right) => left.localeCompare(right)),
    );
    expect(model.diagnostics.every((diagnostic) => diagnostic.code === 'ui-gen(visual-sync)')).toBe(true);
    expect(model.diagnostics.every((diagnostic) => diagnostic.severity === 'warning')).toBe(true);
  });

  // covers: FCT-0039
  it('raises no rule warning for literal and var() values, nor for declarations that are not visual', () => {
    const text = badgeWithStyles(
      `${BADGE_HOST_RULE}\n\t\t.badge { position: relative; color: var(--hz-color-text); translate: -50% 0; }`,
    );

    const model = read(text);

    expect(model.diagnostics).toEqual([]);
    expect(findRule(model, '.badge').values.map(({ key, raw }) => ({ key, raw }))).toEqual([
      { key: 'color', raw: 'var(--hz-color-text)' },
    ]);
  });
});
