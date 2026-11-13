import { useMemo, useState } from "react";
import { startOfDay } from "@/lib/days";
import { copyText } from "@/services/clipboard";
import { getHistory, type HistoryEntry } from "@/services/history";

export type HistoryFilter = "all" | HistoryEntry["kind"];

// Nothing is stored yet, so deleting only lasts until the page is left.
export function useHistory() {
  const [entries, setEntries] = useState(getHistory);
  const [filter, setFilter] = useState<HistoryFilter>("all");
  const [query, setQuery] = useState("");
  // null while entries are not being picked.
  const [selection, setSelection] = useState<ReadonlySet<string> | null>(null);

  const days = useMemo(() => {
    const wanted = query.trim().toLowerCase();
    const byDay = new Map<number, HistoryEntry[]>();
    for (const entry of entries) {
      if (filter !== "all" && entry.kind !== filter) continue;
      const searched = `${entry.text} ${entry.answer ?? ""} ${entry.app}`.toLowerCase();
      if (wanted && !searched.includes(wanted)) continue;
      const day = startOfDay(entry.createdAt);
      byDay.set(day, [...(byDay.get(day) ?? []), entry]);
    }
    return [...byDay].map(([day, items]) => ({ day, items }));
  }, [entries, filter, query]);

  const shown = days.flatMap(({ items }) => items.map((entry) => entry.id));

  const remove = (ids: ReadonlySet<string>) =>
    setEntries((current) => current.filter((entry) => !ids.has(entry.id)));

  return {
    state: {
      hasEntries: entries.length > 0,
      filter,
      query,
      days,
      selection,
      isAllSelected:
        selection !== null && shown.length > 0 && shown.every((id) => selection.has(id)),
    },
    actions: {
      // Picking starts over when the listing changes, so that nothing out of sight is ever deleted.
      filter(next: HistoryFilter) {
        setFilter(next);
        setSelection((current) => current && new Set());
      },
      search(next: string) {
        setQuery(next);
        setSelection((current) => current && new Set());
      },
      copy(entry: HistoryEntry) {
        return copyText(entry.answer ?? entry.text);
      },
      remove(entry: HistoryEntry) {
        remove(new Set([entry.id]));
      },
      removeAll() {
        setEntries([]);
        setSelection(null);
      },
      startSelecting() {
        setSelection(new Set());
      },
      toggle(entry: HistoryEntry) {
        setSelection((current) => {
          const next = new Set(current);
          if (!next.delete(entry.id)) next.add(entry.id);
          return next;
        });
      },
      toggleAll() {
        setSelection((current) =>
          current && shown.every((id) => current.has(id)) ? new Set() : new Set(shown),
        );
      },
      removeSelected() {
        if (selection) remove(selection);
        setSelection(null);
      },
      stopSelecting() {
        setSelection(null);
      },
    },
  };
}
