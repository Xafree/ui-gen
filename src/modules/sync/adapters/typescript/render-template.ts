import ts from 'typescript';
import type { ExpressionShape, ParsedTemplateNode, TemplateExpression } from '../../domain/model/template-syntax.js';
import { readHtmlTemplate, type HtmlTemplateSource } from '../parse5/html-template-reader.js';

function isHtmlTemplate(expression: ts.Expression): expression is ts.TaggedTemplateExpression {
  return (
    ts.isTaggedTemplateExpression(expression) && ts.isIdentifier(expression.tag) && expression.tag.text === 'html'
  );
}

function isEmptyBranch(expression: ts.Expression): boolean {
  if (ts.isIdentifier(expression)) {
    return expression.text === 'nothing' || expression.text === 'undefined';
  }
  return (
    expression.kind === ts.SyntaxKind.NullKeyword || (ts.isStringLiteralLike(expression) && expression.text === '')
  );
}

function representableBranch(branch: ts.Expression, sourceFile: ts.SourceFile): ParsedTemplateNode[] | null {
  if (isEmptyBranch(branch)) {
    return [];
  }
  return isHtmlTemplate(branch) ? readHtmlTemplate(htmlTemplateSource(branch, sourceFile)) : null;
}

function conditionalShape(expression: ts.ConditionalExpression, sourceFile: ts.SourceFile): ExpressionShape {
  const whenTrue = representableBranch(expression.whenTrue, sourceFile);
  const whenFalse = representableBranch(expression.whenFalse, sourceFile);
  if (whenTrue === null || whenFalse === null) {
    return { kind: 'other' };
  }
  return { kind: 'conditional', condition: expression.condition.getText(sourceFile), whenTrue, whenFalse };
}

function shapeOf(expression: ts.Expression, sourceFile: ts.SourceFile): ExpressionShape {
  if (
    ts.isPropertyAccessExpression(expression) &&
    expression.expression.kind === ts.SyntaxKind.ThisKeyword &&
    ts.isIdentifier(expression.name)
  ) {
    return { kind: 'this-property', name: expression.name.text };
  }
  return ts.isConditionalExpression(expression) ? conditionalShape(expression, sourceFile) : { kind: 'other' };
}

function unwrapParentheses(expression: ts.Expression): ts.Expression {
  return ts.isParenthesizedExpression(expression) ? unwrapParentheses(expression.expression) : expression;
}

function renderedExpression(classNode: ts.ClassDeclaration): ts.Expression | null {
  const render = classNode.members.find(
    (member): member is ts.MethodDeclaration =>
      ts.isMethodDeclaration(member) && ts.isIdentifier(member.name) && member.name.text === 'render',
  );
  const returned = render?.body?.statements.findLast(ts.isReturnStatement)?.expression;
  return returned === undefined ? null : unwrapParentheses(returned);
}

function interpolationsOf(template: ts.TemplateExpression, sourceFile: ts.SourceFile): TemplateExpression[] {
  const interpolations: TemplateExpression[] = [];
  let opening: ts.Node = template.head;
  for (const span of template.templateSpans) {
    const start = opening.end - 2;
    const end = span.literal.getStart(sourceFile) + 1;
    interpolations.push({
      range: { start, end },
      source: sourceFile.text.slice(start, end),
      shape: shapeOf(span.expression, sourceFile),
    });
    opening = span.literal;
  }
  return interpolations;
}

function htmlTemplateSource(tagged: ts.TaggedTemplateExpression, sourceFile: ts.SourceFile): HtmlTemplateSource {
  const { template } = tagged;
  const contentStart = template.getStart(sourceFile) + 1;
  return {
    text: sourceFile.text.slice(contentStart, template.end - 1),
    offset: contentStart,
    expressions: ts.isTemplateExpression(template) ? interpolationsOf(template, sourceFile) : [],
  };
}

function wholeExpression(expression: ts.Expression, sourceFile: ts.SourceFile): TemplateExpression {
  return {
    range: { start: expression.getStart(sourceFile), end: expression.end },
    source: expression.getText(sourceFile),
    shape: shapeOf(expression, sourceFile),
  };
}

export function readRenderTemplate(classNode: ts.ClassDeclaration, sourceFile: ts.SourceFile): ParsedTemplateNode[] {
  const rendered = renderedExpression(classNode);
  if (rendered === null) {
    return [];
  }
  if (isHtmlTemplate(rendered)) {
    return readHtmlTemplate(htmlTemplateSource(rendered, sourceFile));
  }
  return [{ kind: 'expression', expression: wholeExpression(rendered, sourceFile) }];
}
