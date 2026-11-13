// When clocks change a day is 23 or 25 hours long, so days are stepped by date, never by adding
// hours.

export function startOfDay(time: number): number {
  return new Date(time).setHours(0, 0, 0, 0);
}

export function shiftDays(day: number, count: number): number {
  const date = new Date(day);
  date.setDate(date.getDate() + count);
  return date.setHours(0, 0, 0, 0);
}

export function daysAgo(day: number, now = Date.now()): number {
  let count = 0;
  for (let step = startOfDay(now); step > day; step = shiftDays(step, -1)) count++;
  return count;
}
