import { userEvent } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  BROKEN_TEXT,
  CARD_TEXT,
  EXPRESSION,
  NOT_REPRESENTABLE_MESSAGE,
  brokenChange,
  cardComponent,
  mountCard,
  warning,
} from '../code/code-fixtures.js';
import { htmlElement, isShown, isTextShown, textOf } from '../editor-fixtures.js';
import { itemIds } from '../layers/layer-fixtures.js';
import {
  SECOND_EXPRESSION,
  brokenOpened,
  counters,
  diagnosticCard,
  gutterColor,
  isWarningUnderline,
  lineNumberOf,
  markedLines,
  openProblemsTab,
  plainLines,
  problemEntries,
  readableChange,
  shownLenses,
  syncState,
  twoExpressionsComponent,
  underlineAt,
  underlines,
} from './problems-fixtures.js';

const mounted: Element[] = [];

const SYNTAX_ERROR_MESSAGE = "';' attendu.";

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const element of mounted.splice(0)) element.remove();
});

describe('a non-representable expression in the code', () => {
  // covers: FCT-0039
  it('underlines exactly the expression in warning style and makes it reachable from the keyboard', async () => {
    await mountCard(mounted);

    expect(underlines().map((mark) => mark.textContent)).toEqual([EXPRESSION]);
    const mark = underlineAt(0);
    expect(isWarningUnderline(mark)).toBe(true);
    expect(lineNumberOf(mark)).toBe(12);
    expect(mark.tabIndex).toBe(0);
    const style = getComputedStyle(mark);
    expect([style.textDecorationLine, style.textDecorationStyle]).toEqual(['underline', 'wavy']);
  });

  // covers: FCT-0039
  it('colors the number of the line concerned and writes the message at its end, on that line only', async () => {
    await mountCard(mounted);

    expect(markedLines('has-warning')).toEqual([12]);
    expect(markedLines('has-error')).toEqual([]);
    expect(gutterColor(12)).not.toBe(gutterColor(11));
    expect(shownLenses()).toEqual([{ line: 12, message: NOT_REPRESENTABLE_MESSAGE }]);
  });

  // covers: FCT-0039
  it('opens the detail card on hover with the title, the message and the location, and closes it when the pointer leaves', async () => {
    await mountCard(mounted);
    expect(isShown('#diag-card')).toBe(false);

    await userEvent.hover(underlineAt(0));

    await expect.poll(() => isShown('#diag-card')).toBe(true);
    const card = diagnosticCard();
    expect(card.title).toBe('Expression non représentable');
    expect(card.text).toContain(NOT_REPRESENTABLE_MESSAGE);
    expect(card.meta).toBe('hz-pricing-card.ts, ligne 12, col. 26 · ui-gen(visual-sync)');
    expect(htmlElement('#diag-card').getAttribute('role')).toBe('tooltip');
    expect(htmlElement('#diag-card').classList.contains('ui-diag-card--warning')).toBe(true);

    await userEvent.unhover(underlineAt(0));
    await expect.poll(() => isShown('#diag-card')).toBe(false);
  });

  // covers: FCT-0039
  it('opens the same card when the underline takes the keyboard focus and closes it when the focus leaves', async () => {
    await mountCard(mounted);
    const mark = underlineAt(0);

    mark.focus({ preventScroll: true });

    await expect.poll(() => isShown('#diag-card')).toBe(true);
    expect(diagnosticCard().title).toBe('Expression non représentable');
    expect(diagnosticCard().meta).toBe('hz-pricing-card.ts, ligne 12, col. 26 · ui-gen(visual-sync)');

    mark.blur();
    await expect.poll(() => isShown('#diag-card')).toBe(false);
  });
});

describe('the Problems tab', () => {
  // covers: FCT-0039
  it('lists the expression once the tab is open, with its message, its code, its line and its column', async () => {
    await mountCard(mounted);
    expect(isShown('#problems')).toBe(false);

    await openProblemsTab();

    expect(isShown('#problems')).toBe(true);
    expect(isShown('#problems-empty')).toBe(false);
    expect(problemEntries()).toHaveLength(1);
    const entry = problemEntries()[0];
    expect(entry?.warning).toBe(true);
    expect(entry?.location).toBe('ligne 12, col. 26');
    expect(entry?.text).toContain(NOT_REPRESENTABLE_MESSAGE);
    expect(entry?.text).toContain('formatPrice(this.price)');
    expect(isShown('#problems-foot')).toBe(true);
    expect(textOf('#problems-foot')).toBe("Le code reste la référence : rien n'est supprimé quand le canvas ne peut pas l'afficher.");
  });

  // covers: FCT-0039
  it('counts the expression on the tab and in the status bar', async () => {
    await mountCard(mounted);

    expect(counters()).toEqual({
      tabCount: '1',
      status: '1 avertissement',
      statusName: '1 avertissement, afficher les problèmes',
    });
    expect(textOf('#tab-problems')).toBe('Problèmes 1');
  });

  // covers: FCT-0039
  it('opens the Problems tab from the status bar counter, even when the code panel is folded', async () => {
    await mountCard(mounted);
    await userEvent.click(htmlElement('#code-collapse'));
    expect(htmlElement('#code-panel').getAttribute('data-collapsed')).toBe('true');

    await userEvent.click(htmlElement('#status-warn'));

    await expect.poll(() => htmlElement('#code-panel').getAttribute('data-collapsed')).toBe('false');
    await expect.poll(() => htmlElement('#tab-problems').getAttribute('aria-selected')).toBe('true');
    expect(htmlElement('#tab-code').getAttribute('aria-selected')).toBe('false');
    expect(isShown('#problems')).toBe(true);
    expect(isShown('#code-pre')).toBe(false);
  });

  // covers: FCT-0039
  it('underlines, counts and lists every expression, in the order of the code', async () => {
    await mountCard(mounted, twoExpressionsComponent());

    expect(underlines().map((mark) => mark.textContent)).toEqual([EXPRESSION, SECOND_EXPRESSION]);
    expect(markedLines('has-warning')).toEqual([12, 13]);
    expect(shownLenses().map(({ line }) => line)).toEqual([12, 13]);
    expect(counters().tabCount).toBe('2');
    expect(counters().status).toMatch(/^2 avertissements?$/);
    expect(counters().statusName).toMatch(/^2 avertissements?, afficher les problèmes$/);

    await openProblemsTab();

    expect(problemEntries().map(({ location }) => location)).toEqual(['ligne 12, col. 26', 'ligne 13, col. 30']);
  });

  // covers: FCT-0039
  it('details in the card the expression under the pointer, not the first one', async () => {
    await mountCard(mounted, twoExpressionsComponent());

    await userEvent.hover(underlineAt(1));

    await expect.poll(() => diagnosticCard().meta).toBe('hz-pricing-card.ts, ligne 13, col. 30 · ui-gen(visual-sync)');
    expect(isShown('#diag-card')).toBe(true);

    await userEvent.hover(underlineAt(0));

    await expect.poll(() => diagnosticCard().meta).toBe('hz-pricing-card.ts, ligne 12, col. 26 · ui-gen(visual-sync)');
  });
});

describe('a component without non-representable code', () => {
  // covers: FCT-0040
  it('shows no counter on the tab nor in the status bar, whose other items stay', async () => {
    await mountCard(mounted, cardComponent([]));

    expect(counters()).toEqual({ tabCount: null, status: null, statusName: null });
    expect(isShown('#tab-problems')).toBe(true);
    expect(textOf('#status-sync-label')).toBe('Synchronisé');
  });

  // covers: FCT-0040
  it('marks nothing in the code', async () => {
    await mountCard(mounted, cardComponent([]));

    expect(underlines()).toEqual([]);
    expect(markedLines('has-warning')).toEqual([]);
    expect(markedLines('has-error')).toEqual([]);
    expect(shownLenses()).toEqual([]);
  });

  // covers: FCT-0040
  it('says « Aucun problème » in the Problems tab, and keeps the note about the code out of it', async () => {
    await mountCard(mounted, cardComponent([]));

    await openProblemsTab();

    expect(isTextShown('Aucun problème')).toBe(true);
    expect(isTextShown("Tout le code de ce composant est représenté dans l'éditeur visuel.")).toBe(true);
    expect(problemEntries()).toEqual([]);
    expect(isShown('#problems-foot')).toBe(false);
  });

  // covers: FCT-0040
  it('removes the marks and the counters when a new version has no problem left', async () => {
    const fake = await mountCard(mounted);
    expect(underlines()).toHaveLength(1);
    expect(counters().tabCount).toBe('1');

    fake.sendChange(readableChange([]));

    await expect.poll(() => underlines()).toHaveLength(0);
    expect(markedLines('has-warning')).toEqual([]);
    expect(shownLenses()).toEqual([]);
    expect(counters()).toEqual({ tabCount: null, status: null, statusName: null });
    expect(textOf('#status-sync-label')).toBe('Synchronisé');
  });
});

describe('a file whose syntax cannot be read', () => {
  // covers: FCT-0043
  it('shows the new text with the error underlined on its line and its message at the end of that line', async () => {
    const fake = await mountCard(mounted);

    fake.sendChange(brokenChange());

    await expect.poll(() => plainLines().join('\n')).toBe(BROKEN_TEXT.slice(0, -1));
    expect(markedLines('has-error')).toEqual([6]);
    expect(markedLines('has-warning')).toEqual([]);
    expect(underlines()).toHaveLength(1);
    expect(isWarningUnderline(underlineAt(0))).toBe(false);
    expect(lineNumberOf(underlineAt(0))).toBe(6);
    expect(shownLenses()).toEqual([{ line: 6, message: SYNTAX_ERROR_MESSAGE }]);
  });

  // covers: FCT-0043
  it('details the error in the card with the syntax error title, its message, its location and its code', async () => {
    const fake = await mountCard(mounted);
    fake.sendChange(brokenChange());
    await expect.poll(() => markedLines('has-error')).toEqual([6]);

    underlineAt(0).focus({ preventScroll: true });

    await expect.poll(() => isShown('#diag-card')).toBe(true);
    const card = diagnosticCard();
    expect(card.title).toBe('Erreur de syntaxe');
    expect(card.text).toContain(SYNTAX_ERROR_MESSAGE);
    expect(card.meta).toBe('hz-pricing-card.ts, ligne 6, col. 25 · ts(1005)');
    expect(htmlElement('#diag-card').classList.contains('ui-diag-card--warning')).toBe(false);
  });

  // covers: FCT-0043
  it('lists the error alone and counts one error, not the warnings of the last readable version', async () => {
    const fake = await mountCard(mounted);

    fake.sendChange(brokenChange());

    await expect.poll(() => counters().status).toBe('1 erreur');
    expect(counters()).toEqual({ tabCount: '1', status: '1 erreur', statusName: '1 erreur, afficher les problèmes' });
    expect(underlines().filter((mark) => isWarningUnderline(mark))).toEqual([]);
    await openProblemsTab();
    expect(problemEntries()).toHaveLength(1);
    const entry = problemEntries()[0];
    expect(entry?.warning).toBe(false);
    expect(entry?.location).toBe('ligne 6, col. 25');
    expect(entry?.text).toContain(SYNTAX_ERROR_MESSAGE);
  });

  // covers: FCT-0043
  it('keeps the layers of the last readable version while the syntax is broken', async () => {
    const fake = await mountCard(mounted);
    await expect.poll(() => itemIds().length).toBeGreaterThan(0);
    const readableLayers = itemIds();

    fake.sendChange(brokenChange());

    await expect.poll(() => markedLines('has-error')).toEqual([6]);
    expect(itemIds()).toEqual(readableLayers);
  });

  // covers: FCT-0043
  it('goes back to the warning of the readable text when the next version can be read again', async () => {
    const fake = await mountCard(mounted);
    fake.sendChange(brokenChange());
    await expect.poll(() => markedLines('has-error')).toEqual([6]);

    fake.sendChange(readableChange([warning()]));

    await expect.poll(() => markedLines('has-error')).toEqual([]);
    await expect.poll(() => plainLines().join('\n')).toBe(CARD_TEXT.slice(0, -1));
    expect(markedLines('has-warning')).toEqual([12]);
    expect(counters()).toEqual({
      tabCount: '1',
      status: '1 avertissement',
      statusName: '1 avertissement, afficher les problèmes',
    });
  });

  // covers: FCT-0043
  it('shows the error and the unreadable state of the sync indicator when the file opens unreadable', async () => {
    await mountCard(mounted, brokenOpened());

    expect(textOf('#tab-code')).toBe('hz-pricing-card.ts');
    expect(plainLines().join('\n')).toBe(BROKEN_TEXT.slice(0, -1));
    expect(markedLines('has-error')).toEqual([6]);
    expect(shownLenses()).toEqual([{ line: 6, message: SYNTAX_ERROR_MESSAGE }]);
    expect(counters()).toEqual({ tabCount: '1', status: '1 erreur', statusName: '1 erreur, afficher les problèmes' });
    expect(syncState()).toEqual({
      code: 'Code illisible : le canvas garde la dernière version lisible',
      codeDot: 'danger',
      status: 'Erreur de syntaxe',
      statusDot: 'danger',
    });
  });
});

describe('the sync indicator', () => {
  // covers: FCT-0043
  it('says the code is in sync with the canvas when it can be read, warnings included', async () => {
    await mountCard(mounted);

    expect(syncState()).toEqual({
      code: 'Synchronisé avec le canvas',
      codeDot: 'success',
      status: 'Synchronisé',
      statusDot: 'success',
    });
    expect(htmlElement('#status-sync').getAttribute('role')).toBe('status');
  });

  // covers: FCT-0043
  it('says the code is unreadable and the canvas keeps the last readable version while the syntax is broken', async () => {
    const fake = await mountCard(mounted);

    fake.sendChange(brokenChange());

    await expect.poll(() => syncState()).toEqual({
      code: 'Code illisible : le canvas garde la dernière version lisible',
      codeDot: 'danger',
      status: 'Erreur de syntaxe',
      statusDot: 'danger',
    });

    fake.sendChange(readableChange([warning()]));

    await expect.poll(() => syncState()).toEqual({
      code: 'Synchronisé avec le canvas',
      codeDot: 'success',
      status: 'Synchronisé',
      statusDot: 'success',
    });
  });

  // covers: FCT-0043
  it('says the analysis restarted while the analyzer restarts, and goes back to in sync when it is ready', async () => {
    const fake = await mountCard(mounted);

    fake.sendStatus('restarting');

    await expect.poll(() => syncState()).toEqual({
      code: 'Analyse du code relancée…',
      codeDot: 'accent',
      status: "Relance de l'analyse…",
      statusDot: 'accent',
    });

    fake.sendStatus('ready');

    await expect.poll(() => syncState()).toEqual({
      code: 'Synchronisé avec le canvas',
      codeDot: 'success',
      status: 'Synchronisé',
      statusDot: 'success',
    });
  });
});
