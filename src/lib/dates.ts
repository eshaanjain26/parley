const DAY_MS = 86_400_000;

const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());

/** Whole calendar days from `d` to `now` (0 = same day), immune to DST hour shifts. */
export function daysAgo(d: Date, now: Date): number {
  return Math.round((startOfDay(now).getTime() - startOfDay(d).getTime()) / DAY_MS);
}

/** "Today", "Yesterday", "Monday, Oct 19" this year, or "Oct 19, 2025" for older years. */
export function dayLabel(ms: number, now = new Date(), locale?: string): string {
  const d = new Date(ms);
  const days = daysAgo(d, now);
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  if (d.getFullYear() === now.getFullYear()) {
    return d.toLocaleDateString(locale, { weekday: "long", month: "short", day: "numeric" });
  }
  return d.toLocaleDateString(locale, { month: "short", day: "numeric", year: "numeric" });
}

/** "1:07 PM" in the user's locale. */
export const timeOfDay = (ms: number, locale?: string) =>
  new Date(ms).toLocaleTimeString(locale, { hour: "numeric", minute: "2-digit" });

export interface DayGroup<T> {
  key: string;
  label: string;
  items: T[];
}

/** Groups items (already newest first) into calendar days, keeping their order. */
export function groupByDay<T extends { startedAtMs: number }>(items: T[], now = new Date(), locale?: string): DayGroup<T>[] {
  const groups: DayGroup<T>[] = [];
  for (const item of items) {
    const d = startOfDay(new Date(item.startedAtMs));
    const key = `${d.getFullYear()}-${d.getMonth() + 1}-${d.getDate()}`;
    const last = groups[groups.length - 1];
    if (last && last.key === key) last.items.push(item);
    else groups.push({ key, label: dayLabel(item.startedAtMs, now, locale), items: [item] });
  }
  return groups;
}

/** "Wednesday, October 21" for the Home header. */
export const longDate = (now = new Date(), locale?: string) =>
  now.toLocaleDateString(locale, { weekday: "long", month: "long", day: "numeric" });

/** "Good morning" / "Good afternoon" / "Good evening". */
export function greeting(now = new Date()): string {
  const h = now.getHours();
  if (h < 12) return "Good morning";
  if (h < 18) return "Good afternoon";
  return "Good evening";
}
