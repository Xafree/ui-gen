import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NOTICE_REQUESTED, type NoticeRequestedDetail } from '../../../src/renderer/studio-events.js';
import { PRICING_CARD, fakeEditorApi, mountEditor, openedMarks, textOf } from './editor-fixtures.js';

const mounted: Element[] = [];

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const element of mounted.splice(0)) element.remove();
});

describe('<ui-editor> open failure', () => {
  it('tells the component cannot be read, with its path, and shows no opened component', async () => {
    const fake = fakeEditorApi({
      lastOpened: () => Promise.resolve(PRICING_CARD.path),
      open: () => Promise.reject(new Error('EACCES')),
    });
    const notices: NoticeRequestedDetail[] = [];
    document.addEventListener(NOTICE_REQUESTED, (event) => {
      if (event instanceof CustomEvent) notices.push(event.detail as NoticeRequestedDetail);
    });

    await mountEditor(fake.api, mounted);

    await expect.poll(() => notices).toHaveLength(1);
    expect(notices[0]?.notice).toEqual({
      tone: 'danger',
      title: "Ce composant ne s'ouvre pas",
      message: `Le fichier ${PRICING_CARD.path} n'a pas pu être lu. Rien n'a été modifié.`,
    });
    expect(textOf('#frame-name')).toBe('');
    expect(openedMarks()).toEqual([]);
  });
});
