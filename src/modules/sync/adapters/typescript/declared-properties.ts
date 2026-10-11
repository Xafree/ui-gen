import ts from 'typescript';
import type { DeclaredProperty } from '../../domain/model/component-model.js';

function reactiveDecorator(member: ts.PropertyDeclaration): DeclaredProperty['decorator'] | null {
  for (const { expression } of ts.getDecorators(member) ?? []) {
    if (ts.isCallExpression(expression) && ts.isIdentifier(expression.expression)) {
      const { text } = expression.expression;
      if (text === 'property' || text === 'state') {
        return text;
      }
    }
  }
  return null;
}

function stringLiteralUnion(type: ts.TypeNode | undefined): string[] {
  if (type === undefined || !ts.isUnionTypeNode(type)) {
    return [];
  }
  const values: string[] = [];
  for (const member of type.types) {
    if (!ts.isLiteralTypeNode(member) || !ts.isStringLiteral(member.literal)) {
      return [];
    }
    values.push(member.literal.text);
  }
  return values;
}

function readProperty(member: ts.ClassElement, sourceFile: ts.SourceFile): DeclaredProperty | null {
  if (!ts.isPropertyDeclaration(member) || !(ts.isIdentifier(member.name) || ts.isStringLiteral(member.name))) {
    return null;
  }
  const decorator = reactiveDecorator(member);
  if (decorator === null) {
    return null;
  }
  return {
    name: member.name.text,
    decorator,
    type: member.type?.getText(sourceFile) ?? null,
    literalValues: stringLiteralUnion(member.type),
    defaultValue: member.initializer?.getText(sourceFile) ?? null,
    range: { start: member.getStart(sourceFile), end: member.end },
  };
}

export function readDeclaredProperties(classNode: ts.ClassDeclaration, sourceFile: ts.SourceFile): DeclaredProperty[] {
  return classNode.members.flatMap((member) => {
    const property = readProperty(member, sourceFile);
    return property === null ? [] : [property];
  });
}
