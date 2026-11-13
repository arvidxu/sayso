import { memo, useEffect, useState } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  ArrowDown01Icon,
  BubbleChatSparkIcon,
  Copy01Icon,
  Delete02Icon,
  Download01Icon,
  MoreHorizontalIcon,
  Tick02Icon,
} from "@hugeicons/core-free-icons";
import { AudioPlayer } from "@/components/history/AudioPlayer";
import { Deferred } from "@/components/shared/Deferred";
import { IconButton } from "@/components/shared/IconButton";
import { MenuButton } from "@/components/shared/MenuButton";
import { ROW } from "@/components/shared/RowList";
import { Button } from "@/components/ui/button";
import { ButtonGroup } from "@/components/ui/button-group";
import { Checkbox } from "@/components/ui/checkbox";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";
import { Item, ItemActions, ItemContent, ItemMedia } from "@/components/ui/item";
import { cn } from "@/lib/utils";
import type { HistoryEntry } from "@/services/history";

const LONG = 220;

type EntryLabels = {
  answer: string;
  more: string;
  copy: string;
  copied: string;
  exportRecording: string;
  delete: string;
  showMore: string;
  showLess: string;
  play: string;
  pause: string;
  position: string;
};

export const EntryRow = memo(function EntryRow({
  entry,
  time,
  labels,
  isSelected,
  onToggle,
  onShowAnswer,
  onCopy,
  onDelete,
}: {
  entry: HistoryEntry;
  time: string;
  labels: EntryLabels;
  // Undefined while entries are not being picked.
  isSelected?: boolean;
  onToggle: () => void;
  onShowAnswer: () => void;
  onCopy: () => Promise<void>;
  onDelete: () => void;
}) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [isCopied, setIsCopied] = useState(false);

  useEffect(() => {
    if (!isCopied) return;
    const timer = setTimeout(() => setIsCopied(false), 1500);
    return () => clearTimeout(timer);
  }, [isCopied]);
  const isLong = entry.text.length > LONG;
  const clamp = isLong && !isExpanded ? "line-clamp-3" : undefined;

  const isPicking = isSelected !== undefined;

  return (
    // While entries are being picked, the whole row picks it.
    <Item
      size="sm"
      className={cn(
        ROW,
        "relative items-start py-3",
        isPicking && "cursor-pointer",
        isSelected && "bg-foreground/6",
      )}
      onClick={isPicking ? onToggle : undefined}
    >
      {/* Wide enough for a 12-hour time with its marker, such as 10:45 PM. */}
      <ItemMedia className="w-18 justify-start pt-0.5 text-[13px] whitespace-nowrap text-muted-foreground tabular-nums">
        {isPicking ? (
          <Checkbox
            aria-label={`${time} ${entry.text.slice(0, 40)}`}
            checked={isSelected}
            onCheckedChange={onToggle}
            onClick={(event) => event.stopPropagation()}
          />
        ) : (
          time
        )}
      </ItemMedia>
      <ItemContent className="min-w-0 gap-1.5">
        <p className={cn("text-sm leading-[22px] select-text", clamp)}>{entry.text}</p>
        {entry.hasAudio && (
          <AudioPlayer
            id={entry.id}
            durationMs={entry.durationMs}
            disabled={isPicking}
            labels={{ play: labels.play, pause: labels.pause, position: labels.position }}
          />
        )}
        {/* While rows are being picked the links are hidden, not removed, so no row changes height. */}
        <div className="flex h-7 items-center gap-3 text-xs text-muted-foreground tabular-nums">
          <span>{entry.app}</span>
          {entry.answer && (
            <Button
              variant="secondary"
              size="xs"
              className={cn("history-meta-action answer-action", isPicking && "invisible")}
              onClick={onShowAnswer}
            >
              <HugeiconsIcon
                icon={BubbleChatSparkIcon}
                strokeWidth={1.8}
                data-icon="inline-start"
              />
              {labels.answer}
            </Button>
          )}
          {isLong && (
            <Button
              variant="ghost"
              size="xs"
              aria-expanded={isExpanded}
              className={cn("history-meta-action expand-action", isPicking && "invisible")}
              onClick={() => setIsExpanded((current) => !current)}
            >
              {isExpanded ? labels.showLess : labels.showMore}
              <HugeiconsIcon
                icon={ArrowDown01Icon}
                strokeWidth={1.8}
                data-icon="inline-end"
                className={cn("transition-transform duration-150", isExpanded && "rotate-180")}
              />
            </Button>
          )}
          <ItemActions
            className={cn(
              "ml-auto opacity-0 transition-[opacity,translate] duration-150 ease-out translate-x-1 group-hover/item:translate-x-0 group-hover/item:opacity-100 focus-within:translate-x-0 focus-within:opacity-100 has-aria-expanded:translate-x-0 has-aria-expanded:opacity-100",
              isPicking && "invisible",
            )}
          >
            {/* Two tooltips and a menu for every row are costly to build, and no row shows them
                until it is pointed at. */}
            <Deferred>
              <ButtonGroup className="entry-actions rounded-full p-0.5">
                <IconButton
                  size="icon-xs"
                  icon={isCopied ? Tick02Icon : Copy01Icon}
                  label={isCopied ? labels.copied : labels.copy}
                  className={cn("size-6 [&_svg]:size-3.5!", isCopied && "text-tone-green")}
                  onClick={() => {
                    onCopy().then(() => setIsCopied(true), console.error);
                  }}
                />
                <DropdownMenu>
                  <MenuButton
                    size="icon-xs"
                    icon={MoreHorizontalIcon}
                    label={labels.more}
                    className="size-6 [&_svg]:size-3.5!"
                  />
                  <DropdownMenuContent align="end" className="min-w-44">
                    {/* Exporting waits for recording to be built; until then it cannot be chosen. */}
                    <DropdownMenuItem disabled>
                      <HugeiconsIcon icon={Download01Icon} strokeWidth={1.5} />
                      {labels.exportRecording}
                    </DropdownMenuItem>
                    <DropdownMenuSeparator />
                    <DropdownMenuItem variant="destructive" onClick={onDelete}>
                      <HugeiconsIcon icon={Delete02Icon} strokeWidth={1.5} />
                      {labels.delete}
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </ButtonGroup>
            </Deferred>
          </ItemActions>
        </div>
      </ItemContent>
    </Item>
  );
}, sameRow);

// A row is drawn again only when what it shows changes. Its handlers are new functions on every
// render of the page, but they do the same thing to the same entry, so they are not compared.
function sameRow(
  before: { entry: HistoryEntry; time: string; labels: EntryLabels; isSelected?: boolean },
  after: { entry: HistoryEntry; time: string; labels: EntryLabels; isSelected?: boolean },
) {
  return (
    before.entry === after.entry &&
    before.time === after.time &&
    before.labels === after.labels &&
    before.isSelected === after.isSelected
  );
}
