// Recording is not built yet, so nothing listens for these keys, and they are a first proposal.
export type Mode = "dictate" | "translate" | "ask";

export const SHORTCUTS: Record<Mode, readonly string[]> = {
  dictate: ["Fn"],
  translate: ["Fn", "Shift"],
  ask: ["Fn", "Space"],
};
