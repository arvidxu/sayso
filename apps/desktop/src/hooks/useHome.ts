import { useEffect, useState, useSyncExternalStore } from "react";
import { getWords, loadDictionary, subscribeDictionary } from "@/services/dictionary";
import { shiftDays, startOfDay } from "@/lib/days";
import { countWords, getHistory, type HistoryEntry } from "@/services/history";
import { app } from "../../../../product.conf.json";
import { SHORTCUTS, type Mode } from "@/services/shortcuts";

// In the order SHORTCUTS lists them; the page shows the first one apart.
const MODES = Object.keys(SHORTCUTS) as Mode[];

const words = (entries: readonly HistoryEntry[]) =>
  entries.reduce((sum, entry) => sum + countWords(entry.text), 0);

export function useHome() {
  const [entries] = useState(getHistory);
  const dictionary = useSyncExternalStore(subscribeDictionary, getWords);
  // "Time saved" is measured against typingWordsPerMinute.
  const { periodDays, chartDays, typingWordsPerMinute } = app.home;

  useEffect(() => {
    loadDictionary().catch(console.error);
  }, []);

  const startOfToday = startOfDay(Date.now());
  const dictated = entries.filter((entry) => entry.kind === "dictation");
  // The start of the day that makes today the last of `days` days.
  const since = (days: number) => shiftDays(startOfToday, 1 - days);
  const period = dictated.filter((entry) => entry.createdAt >= since(periodDays));
  const periodWords = words(period);
  const durationMs = period.reduce((sum, entry) => sum + entry.durationMs, 0);

  const days = Array.from({ length: chartDays }, (_, index) => {
    const day = since(chartDays - index);
    return {
      day,
      words: words(dictated.filter((e) => e.createdAt >= day && e.createdAt < shiftDays(day, 1))),
    };
  });
  const latest = days.reduce((sum, { words }) => sum + words, 0);
  const before = words(
    dictated.filter((e) => e.createdAt >= since(2 * chartDays) && e.createdAt < since(chartDays)),
  );

  return {
    state: {
      modes: MODES.map((id) => ({ id, keys: SHORTCUTS[id] })),
      words: periodWords,
      durationMs,
      sessions: period.length,
      pace: durationMs > 0 ? Math.round(periodWords / (durationMs / 60_000)) : 0,
      savedMs: Math.max(0, (periodWords / typingWordsPerMinute) * 60_000 - durationMs),
      learned: dictionary.filter((word) => word.source === "learned").length,
      days,
      trend: before > 0 ? (latest - before) / before : null,
    },
  };
}
