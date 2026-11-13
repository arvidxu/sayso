import { load, type Store } from "@tauri-apps/plugin-store";

// Kept in dictionary.json in the app's data folder, newest first.
export type DictionaryWord = {
  text: string;
  // Added by hand, or learned from what the user corrected. Nothing learns yet, so there are no
  // words of the second kind.
  source: "manual" | "learned";
};

let store: Store | undefined;
// Replaced on every change, never changed in place, so that a reader can tell by comparing.
let words: readonly DictionaryWord[] = [];
let loading: Promise<void> | undefined;
const listeners = new Set<() => void>();

function publish(next: readonly DictionaryWord[]) {
  words = next;
  for (const listener of listeners) listener();
}

function isWord(value: unknown): value is DictionaryWord {
  const word = value as DictionaryWord | null;
  return typeof word?.text === "string" && (word.source === "manual" || word.source === "learned");
}

// Reads the file once. Changes are written to it only after it has been read.
export function loadDictionary() {
  loading ??= (async () => {
    try {
      const opened = await load("dictionary.json");
      const saved = await opened.get("words");
      store = opened;
      publish(Array.isArray(saved) ? saved.filter(isWord) : []);
    } catch (error) {
      console.error(error);
    }
  })();
  return loading;
}

export function getWords(): readonly DictionaryWord[] {
  return words;
}

// Shows the change at once, and writes it to the file if the file has been read.
function save(next: readonly DictionaryWord[]) {
  publish(next);
  store?.set("words", next).catch(console.error);
}

export function hasWord(text: string) {
  const wanted = text.trim().toLowerCase();
  return words.some((word) => word.text.toLowerCase() === wanted);
}

// Returns whether the word was added; one already there, whatever its case, is not added twice.
export function addWord(text: string): boolean {
  const trimmed = text.trim();
  if (!trimmed || hasWord(trimmed)) return false;
  save([{ text: trimmed, source: "manual" }, ...words]);
  return true;
}

export function removeWord(text: string) {
  save(words.filter((word) => word.text !== text));
}

export function removeWords(texts: ReadonlySet<string>) {
  save(words.filter((word) => !texts.has(word.text)));
}

export function subscribeDictionary(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
