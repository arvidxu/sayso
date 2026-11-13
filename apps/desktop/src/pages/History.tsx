import { useMemo, useState } from "react";
import { AnimatePresence } from "motion/react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  CheckmarkSquare02Icon,
  Clock01Icon,
  CloudOffIcon,
  Copy01Icon,
  Delete02Icon,
  MoreHorizontalIcon,
  Cancel01Icon,
} from "@hugeicons/core-free-icons";
import { useTranslation } from "react-i18next";
import { EntryRow } from "@/components/history/EntryRow";
import { Page } from "@/components/layout/Page";
import { EmptyState } from "@/components/shared/EmptyState";
import { Filter } from "@/components/shared/Filter";
import { IconButton } from "@/components/shared/IconButton";
import { MenuButton } from "@/components/shared/MenuButton";
import { CONFIRM_DESTRUCTIVE, MessageDialog } from "@/components/shared/MessageDialog";
import { SearchField } from "@/components/shared/SearchField";
import { RowList } from "@/components/shared/RowList";
import { ON_BAR, ON_BAR_DANGER, SelectionBar } from "@/components/shared/SelectionBar";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";
import { useHistory, type HistoryFilter } from "@/hooks/useHistory";
import { daysAgo } from "@/lib/days";
import { formatDate, formatTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { HistoryEntry } from "@/services/history";

const FILTERS: readonly HistoryFilter[] = ["all", "dictation", "ask"];

export function History() {
  const { state, actions } = useHistory();
  const { t, i18n } = useTranslation();
  const [isConfirmingDeleteAll, setIsConfirmingDeleteAll] = useState(false);
  const [answered, setAnswered] = useState<HistoryEntry | null>(null);
  const { selection } = state;
  const [isConfirmingDelete, setIsConfirmingDelete] = useState(false);

  // The same for every row, so made once and not for each row on each render.
  const labels = useMemo(
    () => ({
      answer: t("history.answer.show"),
      more: t("history.more"),
      copy: t("history.copy"),
      copied: t("history.copied"),
      exportRecording: t("history.exportRecording"),
      delete: t("history.delete"),
      showMore: t("history.showMore"),
      showLess: t("history.showLess"),
      play: t("history.player.play"),
      pause: t("history.player.pause"),
      position: t("history.player.position"),
    }),
    [t],
  );

  const dayLabel = (day: number) => {
    const ago = daysAgo(day);
    if (ago === 0) return t("history.today");
    if (ago === 1) return t("history.yesterday");
    return formatDate(day, i18n.language);
  };

  const filters = (
    <Filter
      aria-label={t("history.filter")}
      value={state.filter}
      onValueChange={actions.filter}
      options={FILTERS.map((value) => ({
        value,
        label: t(`history.filters.${value}`),
      }))}
    />
  );
  // The top of the page is the same with and without entries.
  const actionsRow = (
    <>
      {/* Where syncing will be turned on; there is nothing to sync with yet. */}
      <IconButton icon={CloudOffIcon} label={t("history.cloudSync")} disabled />
      <DropdownMenu>
        <MenuButton icon={MoreHorizontalIcon} label={t("history.more")} />
        <DropdownMenuContent align="end" className="min-w-44">
          <DropdownMenuItem
            disabled={!state.hasEntries}
            onClick={selection ? actions.stopSelecting : actions.startSelecting}
          >
            <HugeiconsIcon icon={CheckmarkSquare02Icon} strokeWidth={1.5} />
            {selection ? t("history.stopSelecting") : t("history.select")}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            variant="destructive"
            disabled={!state.hasEntries}
            onClick={() => setIsConfirmingDeleteAll(true)}
          >
            <HugeiconsIcon icon={Delete02Icon} strokeWidth={1.5} />
            {t("history.deleteAll.action")}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </>
  );

  return (
    <Page
      title={t("nav.history")}
      actions={actionsRow}
      filters={filters}
      search={
        <SearchField
          label={t("history.search")}
          clearLabel={t("history.closeSearch")}
          value={state.query}
          onChange={actions.search}
        />
      }
      floating={
        <SelectionBar
          open={selection !== null}
          label={t("history.selected", { count: selection?.size ?? 0 })}
        >
          <Button variant="ghost" size="sm" className={ON_BAR} onClick={actions.toggleAll}>
            {state.isAllSelected ? t("history.deselectAll") : t("history.selectAll")}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className={cn(ON_BAR, ON_BAR_DANGER)}
            disabled={!selection?.size}
            onClick={() => setIsConfirmingDelete(true)}
          >
            <HugeiconsIcon icon={Delete02Icon} strokeWidth={1.5} data-icon="inline-start" />
            {t("history.delete")}
          </Button>
          <IconButton
            size="icon-sm"
            icon={Cancel01Icon}
            label={t("history.stopSelecting")}
            className={ON_BAR}
            onClick={actions.stopSelecting}
          />
        </SelectionBar>
      }
    >
      {!state.hasEntries && (
        <EmptyState
          icon={Clock01Icon}
          title={t("history.empty.title")}
          description={t("history.empty.description")}
        />
      )}
      {state.hasEntries && state.days.length === 0 && (
        // Nothing of the kind picked is not the same as nothing matching a search.
        <EmptyState
          description={state.query.trim() ? t("history.noMatch") : t("history.noneOfKind")}
        />
      )}
      <AnimatePresence initial={false}>
        {state.days.map(({ day, items }) => (
          <RowList key={day} title={dayLabel(day)}>
            {items.map((entry) => (
              <EntryRow
                key={entry.id}
                entry={entry}
                time={formatTime(entry.createdAt, i18n.language)}
                labels={labels}
                isSelected={selection ? selection.has(entry.id) : undefined}
                onToggle={() => actions.toggle(entry)}
                onShowAnswer={() => setAnswered(entry)}
                onCopy={() => actions.copy(entry)}
                onDelete={() => actions.remove(entry)}
              />
            ))}
          </RowList>
        ))}
      </AnimatePresence>

      {/* Room for the bar that floats over the bottom while entries are picked. */}
      {selection && <div className="h-10" />}

      <MessageDialog
        open={answered !== null}
        onDismiss={() => setAnswered(null)}
        title={answered?.text ?? ""}
        actions={
          <>
            <Button
              variant="outline"
              onClick={() => {
                if (answered) actions.copy(answered).catch(console.error);
              }}
            >
              <HugeiconsIcon icon={Copy01Icon} strokeWidth={1.5} data-icon="inline-start" />
              {t("history.copy")}
            </Button>
            <Button onClick={() => setAnswered(null)}>{t("history.answer.close")}</Button>
          </>
        }
      >
        <p className="leading-6 select-text">{answered?.answer}</p>
      </MessageDialog>

      <MessageDialog
        open={isConfirmingDeleteAll}
        onDismiss={() => setIsConfirmingDeleteAll(false)}
        title={t("history.deleteAll.title")}
        description={t("history.deleteAll.description")}
        actions={
          <>
            <Button variant="outline" onClick={() => setIsConfirmingDeleteAll(false)}>
              {t("history.cancel")}
            </Button>
            <Button
              variant="destructive"
              className={CONFIRM_DESTRUCTIVE}
              onClick={() => {
                actions.removeAll();
                setIsConfirmingDeleteAll(false);
              }}
            >
              {t("history.deleteAll.confirm")}
            </Button>
          </>
        }
      />

      <MessageDialog
        open={isConfirmingDelete}
        onDismiss={() => setIsConfirmingDelete(false)}
        title={t("history.deleteSelected.title", { count: selection?.size ?? 0 })}
        description={t("history.deleteSelected.description")}
        actions={
          <>
            <Button variant="outline" onClick={() => setIsConfirmingDelete(false)}>
              {t("history.cancel")}
            </Button>
            <Button
              variant="destructive"
              className={CONFIRM_DESTRUCTIVE}
              onClick={() => {
                actions.removeSelected();
                setIsConfirmingDelete(false);
              }}
            >
              {t("history.delete")}
            </Button>
          </>
        }
      />
    </Page>
  );
}
