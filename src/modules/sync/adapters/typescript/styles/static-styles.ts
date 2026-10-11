import ts from 'typescript';
import type { TextRange } from '../../../domain/model/text-range.js';
import type { ParsedStyleSheet } from '../../../domain/styles/style-syntax.js';
import { readCssTemplate, type CssTemplateSource } from '../../postcss/css-template-reader.js';

function isStaticStyles(member: ts.ClassElement): member is ts.PropertyDeclaration {
  return (
    ts.isPropertyDeclaration(member) &&
    ts.isIdentifier(member.name) &&
    member.name.text === 'styles' &&
    (member.modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.StaticKeyword) ?? false)
  );
}

function isCssTemplate(expression: ts.Expression): expression is ts.TaggedTemplateExpression {
  return ts.isTaggedTemplateExpression(expression) && ts.isIdentifier(expression.tag) && expression.tag.text === 'css';
}

function interpolationRanges(template: ts.TemplateExpression, sourceFile: ts.SourceFile): TextRange[] {
  let opening: ts.Node = template.head;
  return template.templateSpans.map((span) => {
    const range = { start: opening.end - 2, end: span.literal.getStart(sourceFile) + 1 };
    opening = span.literal;
    return range;
  });
}

function cssTemplateSource({ template }: ts.TaggedTemplateExpression, sourceFile: ts.SourceFile): CssTemplateSource {
  const offset = template.getStart(sourceFile) + 1;
  return {
    text: sourceFile.text.slice(offset, template.end - 1),
    offset,
    interpolations: ts.isTemplateExpression(template) ? interpolationRanges(template, sourceFile) : [],
  };
}

export function readStaticStyles(classNode: ts.ClassDeclaration, sourceFile: ts.SourceFile): ParsedStyleSheet {
  const initializer = classNode.members.find(isStaticStyles)?.initializer;
  return initializer !== undefined && isCssTemplate(initializer)
    ? readCssTemplate(cssTemplateSource(initializer, sourceFile))
    : [];
}
