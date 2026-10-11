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

type TypeAliases = ReadonlyMap<string, ts.TypeNode>;

function typeAliasesOf(sourceFile: ts.SourceFile): TypeAliases {
  return new Map(sourceFile.statements.filter(ts.isTypeAliasDeclaration).map(({ name, type }) => [name.text, type]));
}

function resolvedType(type: ts.TypeNode | undefined, aliases: TypeAliases): ts.TypeNode | undefined {
  if (type !== undefined && ts.isTypeReferenceNode(type) && ts.isIdentifier(type.typeName)) {
    return aliases.get(type.typeName.text);
  }
  return type;
}

function stringLiteralUnion(declared: ts.TypeNode | undefined, aliases: TypeAliases): string[] {
  const type = resolvedType(declared, aliases);
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

function readProperty(
  member: ts.ClassElement,
  sourceFile: ts.SourceFile,
  aliases: TypeAliases,
): DeclaredProperty | null {
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
    literalValues: stringLiteralUnion(member.type, aliases),
    defaultValue: member.initializer?.getText(sourceFile) ?? null,
    range: { start: member.getStart(sourceFile), end: member.end },
  };
}

export function readDeclaredProperties(classNode: ts.ClassDeclaration, sourceFile: ts.SourceFile): DeclaredProperty[] {
  const aliases = typeAliasesOf(sourceFile);
  return classNode.members.flatMap((member) => {
    const property = readProperty(member, sourceFile, aliases);
    return property === null ? [] : [property];
  });
}
