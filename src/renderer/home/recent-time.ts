const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const WEEK = 7 * DAY;
const MONTH = 30 * DAY;

const relativeFormat = new Intl.RelativeTimeFormat('fr-FR', { numeric: 'auto' });
const fullFormat = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'long', timeStyle: 'short' });

export function formatRelativeTime(at: Date, now: Date): string {
  const elapsed = now.getTime() - at.getTime();
  if (elapsed < MINUTE) return "à l'instant";
  if (elapsed < HOUR) return relativeFormat.format(-Math.round(elapsed / MINUTE), 'minute');
  if (elapsed < DAY) return relativeFormat.format(-Math.round(elapsed / HOUR), 'hour');
  if (elapsed < WEEK) return relativeFormat.format(-Math.round(elapsed / DAY), 'day');
  if (elapsed < MONTH) return relativeFormat.format(-Math.round(elapsed / WEEK), 'week');
  return relativeFormat.format(-Math.round(elapsed / MONTH), 'month');
}

export function formatOpenedAt(at: Date): string {
  return `Ouvert le ${fullFormat.format(at)}`;
}
