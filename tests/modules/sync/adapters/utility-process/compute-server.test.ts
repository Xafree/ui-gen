import { afterEach, describe, expect, it, vi } from 'vitest';
import { readComponent } from '../../../../../src/modules/sync/application/read-component';
import { createTypeScriptComponentParser } from '../../../../../src/modules/sync/adapters/typescript/typescript-component-parser';
import { createTypeScriptHighlighter } from '../../../../../src/modules/sync/adapters/typescript/typescript-highlighter';
import { createComputeServer } from '../../../../../src/modules/sync/adapters/utility-process/compute-server';

const COMPONENT = `import { LitElement, html } from 'lit';
import { customElement } from 'lit/decorators.js';

@customElement('hz-badge')
export class HzBadge extends LitElement {
  render() {
    return html\`<span>Pro</span>\`;
  }
}
`;

const parser = createTypeScriptComponentParser();
const highlighter = createTypeScriptHighlighter();

afterEach(() => {
  vi.restoreAllMocks();
});

describe('compute server', () => {
  const serve = createComputeServer({ readModel: (text) => readComponent(text, parser), highlighter });

  it('answers the model of a component', () => {
    const reply = serve({ id: 4, operation: 'read-model', text: COMPONENT });

    expect(reply).toMatchObject({ outcome: 'model', id: 4, model: { tag: 'hz-badge', className: 'HzBadge' } });
    expect('highlight' in reply).toBe(false);
  });

  it('answers the model and the highlighting of a component', () => {
    const reply = serve({ id: 5, operation: 'analyze', text: COMPONENT });

    expect(reply).toMatchObject({ outcome: 'analysis', id: 5, model: { tag: 'hz-badge' } });
    expect(reply.outcome === 'analysis' && reply.highlight.some(({ className }) => className === 'tk-kw')).toBe(true);
  });

  it('answers a failure, without the text, when the analysis throws', () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const failing = createComputeServer({
      readModel: () => {
        throw new Error('compiler crashed');
      },
      highlighter,
    });

    expect(failing({ id: 6, operation: 'analyze', text: COMPONENT })).toEqual({ outcome: 'failed', id: 6 });
    expect(error).toHaveBeenCalledOnce();
  });
});
