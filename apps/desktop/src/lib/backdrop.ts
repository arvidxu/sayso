// The native window background has to follow the backdrop, as it shows in the title bar.
export const BACKDROP_CHANGE = "backdrop-change";

export const BACKDROP_OPACITY = 0.3;
export const BACKDROP_FADE_MS = 200;

// Backdrops stack: a dialog can open over a panel.
let depth = 0;

export function backdropDepth() {
  return depth;
}

export function showBackdrop() {
  depth += 1;
  dispatchEvent(new Event(BACKDROP_CHANGE));
  return () => {
    depth -= 1;
    dispatchEvent(new Event(BACKDROP_CHANGE));
  };
}
