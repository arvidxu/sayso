import assert from "node:assert/strict";
import { test } from "node:test";
import { daysAgo, shiftDays, startOfDay } from "../src/lib/days.ts";
import { countWords } from "../src/services/history.ts";

test("counts words in scripts with and without spaces, and with combining marks", () => {
  assert.equal(countWords("Can we move the standup to 9:30?"), 8);
  assert.equal(countWords("我用React写代码"), 5);
  assert.equal(countWords("สวัสดีครับ ผมชื่อสมชาย"), 5);
  assert.equal(countWords("मैं हिन्दी बोलता हूँ"), 4);
  assert.equal(countWords("…，。 "), 0);
});

test("steps through days by date, also across a clock change", () => {
  // In a zone that moves its clocks, the day they move is 23 or 25 hours long.
  const zone = process.env.TZ;
  process.env.TZ = "America/Los_Angeles";
  try {
    const today = startOfDay(new Date(2026, 2, 10, 15).getTime());
    const week = Array.from({ length: 7 }, (_, index) => new Date(shiftDays(today, index - 6)));
    assert.deepEqual(
      week.map((day) => [day.getDate(), day.getHours()]),
      [4, 5, 6, 7, 8, 9, 10].map((date) => [date, 0]),
    );
    assert.equal(daysAgo(shiftDays(today, -6), today), 6);
    assert.equal(daysAgo(today, today + 1000), 0);
  } finally {
    process.env.TZ = zone;
  }
});
