import { describe, expect, it } from 'vitest';
import { formatOpenedAt, formatRelativeTime } from '../../../src/renderer/home/recent-time.js';

const NOW = new Date('2026-10-11T12:00:00.000Z');
const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

function ago(milliseconds: number): Date {
  return new Date(NOW.getTime() - milliseconds);
}

describe('formatRelativeTime', () => {
  it.each([
    ['the same instant', 0],
    ['59 seconds', 59 * SECOND],
  ])('reads « à l\'instant » for %s ago', (_label, elapsed) => {
    expect(formatRelativeTime(ago(elapsed), NOW)).toBe("à l'instant");
  });

  it('reads « à l\'instant » for a date slightly ahead of the clock', () => {
    expect(formatRelativeTime(new Date(NOW.getTime() + 30 * SECOND), NOW)).toBe("à l'instant");
  });

  it.each([
    [60 * SECOND, 'il y a 1 minute'],
    [90 * SECOND, 'il y a 2 minutes'],
    [6 * MINUTE, 'il y a 6 minutes'],
    [59 * MINUTE, 'il y a 59 minutes'],
  ])('counts minutes under an hour (%i ms ago)', (elapsed, expected) => {
    expect(formatRelativeTime(ago(elapsed), NOW)).toBe(expected);
  });

  it.each([
    [HOUR, 'il y a 1 heure'],
    [3 * HOUR, 'il y a 3 heures'],
    [23 * HOUR, 'il y a 23 heures'],
  ])('counts hours under a day (%i ms ago)', (elapsed, expected) => {
    expect(formatRelativeTime(ago(elapsed), NOW)).toBe(expected);
  });

  it.each([
    [DAY + 3 * HOUR, 'hier'],
    [2 * DAY, 'avant-hier'],
    [4 * DAY, 'il y a 4 jours'],
    [6 * DAY, 'il y a 6 jours'],
  ])('counts days under a week with French day words (%i ms ago)', (elapsed, expected) => {
    expect(formatRelativeTime(ago(elapsed), NOW)).toBe(expected);
  });

  it.each([
    [7 * DAY, 'la semaine dernière'],
    [23 * DAY, 'il y a 3 semaines'],
    [29 * DAY, 'il y a 4 semaines'],
  ])('counts weeks under thirty days (%i ms ago)', (elapsed, expected) => {
    expect(formatRelativeTime(ago(elapsed), NOW)).toBe(expected);
  });

  it.each([
    [30 * DAY, 'le mois dernier'],
    [90 * DAY, 'il y a 3 mois'],
    [400 * DAY, 'il y a 13 mois'],
  ])('counts months from thirty days on, without years (%i ms ago)', (elapsed, expected) => {
    expect(formatRelativeTime(ago(elapsed), NOW)).toBe(expected);
  });
});

describe('formatOpenedAt', () => {
  it.each([
    [new Date(2026, 9, 11, 14, 30), 'Ouvert le 11 octobre 2026 à 14:30'],
    [new Date(2026, 0, 5, 9, 5), 'Ouvert le 5 janvier 2026 à 09:05'],
  ])('writes the long French date and the short time of %s', (at, expected) => {
    expect(formatOpenedAt(at)).toBe(expected);
  });
});
