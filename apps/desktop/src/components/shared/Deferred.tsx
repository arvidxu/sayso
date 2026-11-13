import { useDeferredValue, type ReactNode } from "react";

// Fills in its children in a second, low-priority render, for parts that are costly to build and
// not needed in the first frame.
export function Deferred({ children }: { children: ReactNode }) {
  const isReady = useDeferredValue(true, false);
  return isReady ? children : null;
}
