import type { ComponentModel, NodeId } from '../../../sync/application/component-model.js';

type StyleRule = ComponentModel['styles'][number];

export type InspectorKey = StyleRule['values'][number]['key'];

export type InspectorValues = Readonly<Partial<Record<InspectorKey, string>>>;

function appliesTo(rule: StyleRule, model: ComponentModel, nodeId: NodeId): boolean {
  return rule.state === null && rule.targets.includes(nodeId) && (rule.variant === null || rule.variant.property === model.variantProperty);
}

function isCommon(rule: StyleRule): boolean {
  return rule.variant === null;
}

function matchesVariant(rule: StyleRule, variant: string | null): boolean {
  return rule.variant === null || rule.variant.value === variant;
}

export function inspectorValues(model: ComponentModel, nodeId: NodeId, variant: string | null): InspectorValues {
  const rules = model.styles.filter((rule) => appliesTo(rule, model, nodeId) && matchesVariant(rule, variant));
  const ordered = [...rules.filter(isCommon), ...rules.filter((rule) => !isCommon(rule))];
  const shown: Partial<Record<InspectorKey, string>> = {};
  for (const rule of ordered) {
    for (const value of rule.values) shown[value.key] = value.token ?? value.raw;
  }
  return shown;
}
