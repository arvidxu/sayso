import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Bar, BarChart, Cell, YAxis } from "recharts";
import { PauseIcon, PlayIcon } from "@hugeicons/core-free-icons";
import { Deferred } from "@/components/shared/Deferred";
import { Button } from "@/components/ui/button";
import { ChartContainer, type ChartConfig } from "@/components/ui/chart";
import { Slider } from "@/components/ui/slider";
import { formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";

// Only one recording plays at a time: starting one pauses the one before it.
let playing: string | null = null;
const listeners = new Set<() => void>();
function setPlaying(id: string | null) {
  playing = id;
  for (const listener of listeners) listener();
}
function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

const BARS = 44;

// The waveform carries no legend or tooltip, so it has nothing to name.
const WAVE = {} satisfies ChartConfig;

// Made up from the entry's id, so each recording keeps its shape.
function waveform(seed: string) {
  let hash = 0;
  for (const char of seed) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  return Array.from({ length: BARS }, (_, index) => {
    hash = (hash * 1103515245 + 12345) >>> 0;
    const noise = (hash % 1000) / 1000;
    const envelope = Math.sin((index / (BARS - 1)) * Math.PI) * 0.55 + 0.45;
    return Math.max(0.16, Math.min(1, envelope * (0.45 + noise * 0.75)));
  });
}

type PlayerLabels = { play: string; pause: string; position: string };

// Recording is not built yet, so there is no file to play: playback is timed against the
// recording's length.
export function AudioPlayer({
  id,
  durationMs,
  labels,
  disabled = false,
}: {
  id: string;
  durationMs: number;
  labels: PlayerLabels;
  disabled?: boolean;
}) {
  const isPlaying = useSyncExternalStore(subscribe, () => playing === id);
  const [position, setPosition] = useState(0);
  const positionRef = useRef(0);
  // Each bar reaches as far above the middle as below it.
  const bars = useMemo(
    () => waveform(id).map((height, index) => ({ index, span: [-height, height] })),
    [id],
  );

  const seek = (milliseconds: number) => {
    const next = Math.max(0, Math.min(durationMs, milliseconds));
    positionRef.current = next;
    setPosition(next);
  };

  useEffect(() => {
    if (!isPlaying) return;
    let last = performance.now();
    let frame = requestAnimationFrame(function step(now) {
      const next = positionRef.current + (now - last);
      last = now;
      if (next >= durationMs) {
        seek(0);
        setPlaying(null);
        return;
      }
      seek(next);
      frame = requestAnimationFrame(step);
    });
    return () => cancelAnimationFrame(frame);
  }, [isPlaying, durationMs]);

  // A player that cannot be used cannot be paused either, so it stops.
  useEffect(() => {
    if (disabled && playing === id) setPlaying(null);
  }, [disabled, id]);

  useEffect(
    () => () => {
      if (playing === id) setPlaying(null);
    },
    [id],
  );

  const toggle = () => setPlaying(isPlaying ? null : id);
  const played = durationMs > 0 ? Math.floor((position / durationMs) * BARS) : 0;
  const chart = useMemo(
    () => (
      <ChartContainer
        config={WAVE}
        className="audio-wave pointer-events-none absolute inset-0 aspect-auto size-full"
        aria-hidden
      >
        <BarChart
          data={bars}
          margin={{ top: 0, right: 0, bottom: 0, left: 0 }}
          barCategoryGap={1}
          accessibilityLayer={false}
        >
          <YAxis hide domain={[-1, 1]} />
          <Bar dataKey="span" radius={99} isAnimationActive={false}>
            {bars.map(({ index }) => (
              <Cell key={index} className={index < played ? "is-played" : undefined} />
            ))}
          </Bar>
        </BarChart>
      </ChartContainer>
    ),
    [bars, played],
  );
  const hasStarted = isPlaying || position > 0;

  return (
    <div
      className={cn(
        "audio-player flex h-9 w-full max-w-[22rem] items-center gap-2.5 rounded-full py-1 pr-3.5 pl-1",
        isPlaying && "is-playing",
        disabled && "pointer-events-none opacity-50",
      )}
      // Out of reach of the keyboard too while the row is being picked.
      inert={disabled}
      onClick={(event) => event.stopPropagation()}
    >
      <Button
        size="icon-sm"
        variant={isPlaying ? "default" : "secondary"}
        aria-label={isPlaying ? labels.pause : labels.play}
        className="size-7 rounded-full"
        onClick={toggle}
      >
        <HugeiconsIcon
          icon={isPlaying ? PauseIcon : PlayIcon}
          strokeWidth={2}
          className={cn("size-3.5", !isPlaying && "translate-x-px")}
        />
      </Button>
      <div className="relative h-6 min-w-0 flex-1">
        {/* Drawn again only when playback passes another bar, not on every frame. */}
        <Deferred>{chart}</Deferred>
        {/* The slider is costly to build and only matters once the player is used. */}
        <Deferred>
          <Slider
            aria-label={labels.position}
            className="audio-slider absolute inset-0"
            min={0}
            max={durationMs}
            step={1000}
            largeStep={5000}
            value={[position]}
            onValueChange={(value) => seek(Array.isArray(value) ? value[0] : value)}
          />
        </Deferred>
      </div>
      <span className="shrink-0 text-[11px] text-muted-foreground tabular-nums">
        {hasStarted ? `${formatDuration(position)} / ` : ""}
        {formatDuration(durationMs)}
      </span>
    </div>
  );
}
