export type HistoryEntry = {
  id: string;
  // Text that was typed for the user, or a question they asked.
  kind: "dictation" | "ask";
  text: string;
  answer?: string;
  // Milliseconds since 1970.
  createdAt: number;
  durationMs: number;
  // The app the user was in.
  app: string;
  // Whether the recording was kept on this computer.
  hasAudio: boolean;
};

// Nothing is recorded yet, so there is nothing to read.
export function getHistory(): HistoryEntry[] {
  return [];
}

// Words as the system's word breaking finds them, in any language, with or without spaces.
const wordSegmenter = new Intl.Segmenter(undefined, { granularity: "word" });

export function countWords(text: string): number {
  let count = 0;
  for (const segment of wordSegmenter.segment(text)) if (segment.isWordLike) count++;
  return count;
}
