import ts from 'typescript';

export interface ComponentClass {
  readonly tag: string;
  readonly className: string;
  readonly node: ts.ClassDeclaration;
}

interface Registration {
  readonly tag: string;
  readonly className: string;
}

function isIdentifierNamed(node: ts.Node, name: string): boolean {
  return ts.isIdentifier(node) && node.text === name;
}

function decoratedTag(node: ts.ClassDeclaration): string | null {
  for (const { expression } of ts.getDecorators(node) ?? []) {
    if (ts.isCallExpression(expression) && isIdentifierNamed(expression.expression, 'customElement')) {
      const [tag] = expression.arguments;
      if (tag !== undefined && ts.isStringLiteralLike(tag)) {
        return tag.text;
      }
    }
  }
  return null;
}

function isCustomElementsDefine(callee: ts.Expression): boolean {
  if (!ts.isPropertyAccessExpression(callee) || callee.name.text !== 'define') {
    return false;
  }
  const registry = callee.expression;
  return (
    isIdentifierNamed(registry, 'customElements') ||
    (ts.isPropertyAccessExpression(registry) && registry.name.text === 'customElements')
  );
}

function registrations(sourceFile: ts.SourceFile): Registration[] {
  const found: Registration[] = [];
  const visit = (node: ts.Node): void => {
    if (ts.isCallExpression(node) && isCustomElementsDefine(node.expression)) {
      const [tag, constructor] = node.arguments;
      if (tag !== undefined && ts.isStringLiteralLike(tag) && constructor !== undefined && ts.isIdentifier(constructor)) {
        found.push({ tag: tag.text, className: constructor.text });
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return found;
}

export function findComponentClasses(sourceFile: ts.SourceFile): ComponentClass[] {
  const classes = sourceFile.statements.filter(ts.isClassDeclaration);
  const decorated = classes.flatMap((node) => {
    const tag = decoratedTag(node);
    return tag !== null && node.name !== undefined ? [{ tag, className: node.name.text, node }] : [];
  });
  const registered = registrations(sourceFile).flatMap(({ tag, className }) => {
    const node = classes.find((candidate) => candidate.name?.text === className);
    return node === undefined ? [] : [{ tag, className, node }];
  });
  return [...decorated, ...registered];
}
