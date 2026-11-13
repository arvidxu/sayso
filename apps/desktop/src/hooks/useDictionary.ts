import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import {
  addWord,
  getWords,
  hasWord,
  loadDictionary,
  removeWord,
  removeWords,
  subscribeDictionary,
  type DictionaryWord,
} from "@/services/dictionary";

export type DictionaryFilter = "all" | DictionaryWord["source"];

export function useDictionary() {
  const words = useSyncExternalStore(subscribeDictionary, getWords);
  const [filter, setFilter] = useState<DictionaryFilter>("all");
  const [query, setQuery] = useState("");
  const [draft, setDraft] = useState<string | null>(null);
  // null while words are not being picked.
  const [selection, setSelection] = useState<ReadonlySet<string> | null>(null);

  useEffect(() => {
    loadDictionary().catch(console.error);
  }, []);

  const shown = useMemo(() => {
    const wanted = query.trim().toLowerCase();
    return words.filter(
      (word) =>
        (filter === "all" || word.source === filter) && word.text.toLowerCase().includes(wanted),
    );
  }, [words, filter, query]);

  const count = (source: DictionaryWord["source"]) =>
    words.filter((word) => word.source === source).length;

  return {
    state: {
      filter,
      query,
      words: shown,
      counts: { all: words.length, manual: count("manual"), learned: count("learned") },
      // null while the dialog is closed.
      draft,
      isDraftKnown: draft !== null && draft.trim() !== "" && hasWord(draft),
      selection,
      isAllSelected:
        selection !== null && shown.length > 0 && shown.every((word) => selection.has(word.text)),
    },
    actions: {
      // Picking starts over when the listing changes, so that nothing out of sight is ever removed.
      filter(next: DictionaryFilter) {
        setFilter(next);
        setSelection((current) => current && new Set());
      },
      search(next: string) {
        setQuery(next);
        setSelection((current) => current && new Set());
      },
      startAdding() {
        setDraft("");
      },
      type: setDraft,
      cancelAdding() {
        setDraft(null);
      },
      add() {
        if (draft !== null && addWord(draft)) setDraft(null);
      },
      remove: removeWord,
      removeAll() {
        removeWords(new Set(words.map((word) => word.text)));
        setSelection(null);
      },
      startSelecting() {
        setSelection(new Set());
      },
      toggle(text: string) {
        setSelection((current) => {
          const next = new Set(current);
          if (!next.delete(text)) next.add(text);
          return next;
        });
      },
      toggleAll() {
        setSelection((current) =>
          current && shown.every((word) => current.has(word.text))
            ? new Set()
            : new Set(shown.map((word) => word.text)),
        );
      },
      removeSelected() {
        if (selection) removeWords(selection);
        setSelection(null);
      },
      stopSelecting() {
        setSelection(null);
      },
    },
  };
}
