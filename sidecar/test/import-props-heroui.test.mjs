import { after, afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { closeBrowser } from '../browser.mjs';
import { inspect } from '../inspect.mjs';
import { readInstalledPackage } from '../installed-package.mjs';
import { readProps } from '../props.mjs';
import { render } from '../render.mjs';
import { makeSourcesProject } from './helpers/review-project.mjs';

const LIMITS = { budgetMs: 20_000, maxFiles: 4_000, maxBytes: 64 * 1024 * 1024 };
const REAL_PROJECT = process.env.UIGEN_HEROUI_PROJECT;
const BLUE = 'rgb(51, 102, 153)';

const manifestOf = (name, extra = {}) =>
  `${JSON.stringify({ name, version: '1.0.0', type: 'module', main: 'index.js', types: 'index.d.ts', peerDependencies: { react: '>=18' }, ...extra })}\n`;

const ARIA_DECLARATIONS = `export interface AriaButtonProps {
  isDisabled?: boolean;
  slot?: string | null;
  onPress?: (event: unknown) => void;
  children?: unknown;
  className?: string;
}
export declare function ButtonPrimitive(props: AriaButtonProps): unknown;
export type ComponentPropsWithRef<T extends (props: never) => unknown> = Parameters<T>[0] & { ref?: unknown };
`;

const STYLES_DECLARATIONS = `type StringToBoolean<T> = T extends 'true' | 'false' ? boolean : T;
export type VariantProps<C extends { variants: Record<string, Record<string, string>> }> = {
  [K in keyof C['variants']]?: StringToBoolean<keyof C['variants'][K]> | undefined;
};
export declare const buttonVariants: {
  variants: {
    fullWidth: { false: ''; true: 'button--full-width' };
    isIconOnly: { true: 'button--icon-only' };
    size: { lg: 'button--lg'; md: 'button--md'; sm: 'button--sm' };
    variant: { danger: 'button--danger'; ghost: 'button--ghost'; outline: 'button--outline'; primary: 'button--primary' };
  };
};
export type ButtonVariants = VariantProps<typeof buttonVariants>;
`;

const REACT_DECLARATIONS = `import type { ButtonVariants } from 'shape-styles';
import type { ComponentPropsWithRef } from 'shape-aria';
import { ButtonPrimitive } from 'shape-aria';
export declare const BUTTON_GROUP_CHILD = '__button_group_child';
export interface ButtonRootProps extends ComponentPropsWithRef<typeof ButtonPrimitive>, ButtonVariants {
  [BUTTON_GROUP_CHILD]?: boolean;
}
declare const ButtonRoot: {
  (props: ButtonRootProps): unknown;
  displayName: string;
};
export declare const Button: {
  (props: ButtonRootProps): unknown;
  displayName: string;
} & {
  Root: typeof ButtonRoot;
};
`;

const REACT_SOURCE = `import { createElement } from 'react';
export function Button() { return createElement('button', null, 'ok'); }
`;

const SHAPE_PACKAGES = {
  'shape-aria': { 'package.json': manifestOf('shape-aria', { peerDependencies: {} }), 'index.d.ts': ARIA_DECLARATIONS, 'index.js': 'export {};\n' },
  'shape-styles': { 'package.json': manifestOf('shape-styles', { peerDependencies: {} }), 'index.d.ts': STYLES_DECLARATIONS, 'index.js': 'export {};\n' },
  'shape-react': { 'package.json': manifestOf('shape-react'), 'index.d.ts': REACT_DECLARATIONS, 'index.js': REACT_SOURCE },
};

const propsOf = (components, name) => {
  const component = components.find((entry) => entry.name === name);
  assert.ok(component !== undefined, `component ${name} is missing`);
  return component.props;
};

const find = (props, name) => props.find((prop) => prop.name === name);

const sortedValues = (prop) => [...(prop?.values ?? [])].sort();

const assertButtonProps = (props) => {
  const variant = find(props, 'variant');
  assert.equal(variant?.type, 'enum', `variant is missing or not an enum in ${JSON.stringify(props)}`);
  assert.deepEqual(sortedValues(variant), ['danger', 'ghost', 'outline', 'primary']);
  const size = find(props, 'size');
  assert.equal(size?.type, 'enum', `size is missing or not an enum in ${JSON.stringify(props)}`);
  assert.deepEqual(sortedValues(size), ['lg', 'md', 'sm']);
  for (const name of ['fullWidth', 'isIconOnly', 'isDisabled']) {
    assert.equal(find(props, name)?.type, 'boolean', `${name} is missing or not a boolean in ${JSON.stringify(props)}`);
  }
  assert.deepEqual(
    props.filter((prop) => prop.name.startsWith('__')),
    [],
  );
  assert.equal(find(props, 'onPress'), undefined);
};

describe('props of a component whose types come from sibling packages', () => {
  const cleanups = [];

  afterEach(async () => {
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });

  after(closeBrowser);

  const shapeProject = async () => {
    const made = await makeSourcesProject(SHAPE_PACKAGES);
    cleanups.push(made.remove);
    return made;
  };

  // covers: STY-7
  it('reads variant and size literal values and the booleans of a compound component declared through sibling packages', async () => {
    const { dir } = await shapeProject();
    const installed = await readInstalledPackage(dir, 'shape-react');

    const { components } = await readProps(installed, LIMITS);

    assertButtonProps(components.get('Button') ?? []);
  });

  // covers: STY-7
  it('returns the same props through inspect', async () => {
    const { dir } = await shapeProject();

    const result = await inspect({ dir, package: 'shape-react' });

    assertButtonProps(propsOf(result.components, 'Button'));
  });

  // covers: STY-7
  it('does not offer the internal double-underscore prop even when nothing else is readable', async () => {
    const { dir } = await shapeProject();
    const installed = await readInstalledPackage(dir, 'shape-react');

    const { components } = await readProps(installed, LIMITS);

    const names = (components.get('Button') ?? []).map((prop) => prop.name);
    assert.ok(names.length > 0, 'at least one prop is offered');
    assert.ok(!names.includes('__button_group_child'), names.join(', '));
  });

  // covers: STY-7
  it(
    'reads variant and size of the real HeroUI Button',
    { skip: REAL_PROJECT === undefined ? 'set UIGEN_HEROUI_PROJECT to a project folder holding @heroui/react in .uigen/sources' : false },
    async () => {
      const installed = await readInstalledPackage(REAL_PROJECT, '@heroui/react');

      const { components } = await readProps(installed, LIMITS);

      const props = components.get('Button') ?? [];
      const variant = find(props, 'variant');
      const size = find(props, 'size');
      assert.equal(variant?.type, 'enum', JSON.stringify(props));
      assert.ok(variant.values.includes('primary'), JSON.stringify(variant));
      assert.equal(size?.type, 'enum', JSON.stringify(props));
      assert.ok(['sm', 'md', 'lg'].every((value) => size.values.includes(value)), JSON.stringify(size));
      assert.deepEqual(
        props.filter((prop) => prop.name.startsWith('__')),
        [],
      );
    },
  );
});

describe('disabled state of components taking isDisabled', () => {
  const cleanups = [];

  afterEach(async () => {
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });

  after(closeBrowser);

  const STYLE = '.sty-btn { background: #ffffff; } .sty-btn[data-off] { background: #336699; }';

  const LIBRARY = `import { createElement } from 'react';
const style = createElement('style', null, ${JSON.stringify(STYLE)});
export function AriaButton({ isDisabled }) {
  return createElement('button', { className: 'sty-btn', 'data-off': isDisabled ? 'true' : undefined }, style, 'aria');
}
export function NativeButton({ disabled }) {
  return createElement('button', { className: 'sty-btn', 'data-off': disabled ? 'true' : undefined }, style, 'native');
}
export function Echo(props) {
  return createElement('div', null, 'seen:disabled=' + String(props.disabled) + ';isDisabled=' + String(props.isDisabled) + ';variant=' + String(props.variant));
}
`;

  const project = async () => {
    const made = await makeSourcesProject({ 'sty-disabled-lib': { 'package.json': manifestOf('sty-disabled-lib'), 'index.js': LIBRARY } });
    cleanups.push(made.remove);
    return made.dir;
  };

  const renderOf = (dir, component, state, props = {}) =>
    render({ dir, package: 'sty-disabled-lib', component, props, state, theme: 'light' });

  const collect = (node) => [node, ...(node.children ?? []).flatMap(collect)];

  const elementWithText = (snapshot, text) => {
    const found = collect(snapshot.root).find(
      (node) => node.type === 'element' && (node.children ?? []).some((child) => child.type === 'text' && child.text === text),
    );
    assert.ok(found, `element with text ${text} is in the snapshot`);
    return found;
  };

  const seen = (snapshot) => {
    const text = collect(snapshot.root).find((node) => node.type === 'text' && node.text.startsWith('seen:'));
    assert.ok(text, 'the echo text is in the snapshot');
    return text.text;
  };

  // covers: STY-8
  it('passes both disabled and isDisabled as true to a component in the disabled state, keeping the other props', async () => {
    const dir = await project();

    const { snapshot } = await renderOf(dir, 'Echo', 'disabled', { variant: 'primary' });

    assert.equal(seen(snapshot), 'seen:disabled=true;isDisabled=true;variant=primary');
  });

  // covers: STY-8
  it('shows the disabled style of a component that reads only isDisabled', async () => {
    const dir = await project();

    const { snapshot } = await renderOf(dir, 'AriaButton', 'disabled');

    assert.equal(elementWithText(snapshot, 'aria').style.backgroundColor, BLUE);
  });

  // covers: STY-8
  it('shows the disabled style of a component that reads only disabled', async () => {
    const dir = await project();

    const { snapshot } = await renderOf(dir, 'NativeButton', 'disabled');

    assert.equal(elementWithText(snapshot, 'native').style.backgroundColor, BLUE);
  });

  // covers: STY-8
  it('passes neither disabled nor isDisabled in the default state', async () => {
    const dir = await project();

    const { snapshot } = await renderOf(dir, 'Echo', 'default');

    assert.equal(seen(snapshot), 'seen:disabled=undefined;isDisabled=undefined;variant=undefined');
  });
});
