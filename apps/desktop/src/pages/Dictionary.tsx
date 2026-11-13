import { useState } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Add01Icon,
  Book02Icon,
  Cancel01Icon,
  CheckmarkSquare02Icon,
  Delete02Icon,
  MoreHorizontalIcon,
  PencilEdit02Icon,
  SparklesIcon,
} from "@hugeicons/core-free-icons";
import { useTranslation } from "react-i18next";
import { Page } from "@/components/layout/Page";
import { Deferred } from "@/components/shared/Deferred";
import { EmptyState } from "@/components/shared/EmptyState";
import { Filter } from "@/components/shared/Filter";
import { IconButton } from "@/components/shared/IconButton";
import { MenuButton } from "@/components/shared/MenuButton";
import { CONFIRM_DESTRUCTIVE, MessageDialog } from "@/components/shared/MessageDialog";
import { ROW, RowList } from "@/components/shared/RowList";
import { SearchField } from "@/components/shared/SearchField";
import { ON_BAR, ON_BAR_DANGER, SelectionBar } from "@/components/shared/SelectionBar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Item, ItemActions, ItemContent, ItemMedia, ItemTitle } from "@/components/ui/item";
import { useDictionary, type DictionaryFilter } from "@/hooks/useDictionary";
import { cn } from "@/lib/utils";
import type { DictionaryWord } from "@/services/dictionary";

const FILTERS: readonly DictionaryFilter[] = ["all", "manual", "learned"];

function SourceBadge({ source, label }: { source: DictionaryWord["source"]; label: string }) {
  return (
    <Badge
      variant="secondary"
      className={cn("source-badge h-6 px-2.5 text-[11.5px]", `source-${source}`)}
    >
      <HugeiconsIcon
        icon={source === "manual" ? PencilEdit02Icon : SparklesIcon}
        strokeWidth={1.8}
        data-icon="inline-start"
      />
      {label}
    </Badge>
  );
}

export function Dictionary() {
  const { state, actions } = useDictionary();
  const { t } = useTranslation();
  const [isConfirmingDeleteAll, setIsConfirmingDeleteAll] = useState(false);
  const [isConfirmingDelete, setIsConfirmingDelete] = useState(false);
  const isEmpty = state.counts[state.filter] === 0;
  const { selection } = state;
  const isPicking = selection !== null;

  return (
    <Page
      title={t("nav.dictionary")}
      actions={
        <>
          {/* Always there, so the top of the page is the same with and without words. */}
          <DropdownMenu>
            <MenuButton icon={MoreHorizontalIcon} label={t("dictionary.more")} />
            <DropdownMenuContent align="end" className="min-w-44">
              <DropdownMenuItem
                disabled={state.counts.all === 0}
                onClick={isPicking ? actions.stopSelecting : actions.startSelecting}
              >
                <HugeiconsIcon icon={CheckmarkSquare02Icon} strokeWidth={1.5} />
                {isPicking ? t("dictionary.stopSelecting") : t("dictionary.select")}
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                variant="destructive"
                disabled={state.counts.all === 0}
                onClick={() => setIsConfirmingDeleteAll(true)}
              >
                <HugeiconsIcon icon={Delete02Icon} strokeWidth={1.5} />
                {t("dictionary.deleteAll.action")}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <IconButton
            variant="secondary"
            icon={Add01Icon}
            label={t("dictionary.add")}
            className="ml-1 text-primary min-[900px]:hidden"
            onClick={actions.startAdding}
          />
          <Button
            variant="secondary"
            className="ml-1 max-[899px]:hidden"
            onClick={actions.startAdding}
          >
            <HugeiconsIcon
              icon={Add01Icon}
              strokeWidth={1.8}
              data-icon="inline-start"
              className="text-primary"
            />
            {t("dictionary.add")}
          </Button>
        </>
      }
      filters={
        <Filter
          aria-label={t("dictionary.filter")}
          value={state.filter}
          onValueChange={actions.filter}
          options={FILTERS.map((value) => ({
            value,
            label: t(`dictionary.filters.${value}`),
          }))}
        />
      }
      search={
        <SearchField
          label={t("dictionary.search")}
          clearLabel={t("dictionary.closeSearch")}
          value={state.query}
          onChange={actions.search}
        />
      }
      floating={
        <SelectionBar
          open={isPicking}
          label={t("dictionary.selected", { count: selection?.size ?? 0 })}
        >
          <Button variant="ghost" size="sm" className={ON_BAR} onClick={actions.toggleAll}>
            {state.isAllSelected ? t("dictionary.deselectAll") : t("dictionary.selectAll")}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className={cn(ON_BAR, ON_BAR_DANGER)}
            disabled={!selection?.size}
            onClick={() => setIsConfirmingDelete(true)}
          >
            <HugeiconsIcon icon={Delete02Icon} strokeWidth={1.5} data-icon="inline-start" />
            {t("dictionary.delete")}
          </Button>
          <IconButton
            size="icon-sm"
            icon={Cancel01Icon}
            label={t("dictionary.stopSelecting")}
            className={ON_BAR}
            onClick={actions.stopSelecting}
          />
        </SelectionBar>
      }
    >
      {isEmpty && (
        <EmptyState
          icon={Book02Icon}
          title={t(`dictionary.empty.${state.filter}.title`)}
          description={t(`dictionary.empty.${state.filter}.description`)}
        />
      )}
      {!isEmpty && state.words.length === 0 && <EmptyState description={t("dictionary.noMatch")} />}
      {state.words.length > 0 && (
        <RowList key={state.filter}>
          {state.words.map((word) => {
            const isSelected = selection?.has(word.text) ?? false;
            return (
              <Item
                key={word.text}
                size="sm"
                className={cn(ROW, isPicking && "cursor-pointer", isSelected && "bg-foreground/6")}
                onClick={isPicking ? () => actions.toggle(word.text) : undefined}
              >
                {isPicking && (
                  <ItemMedia className="translate-y-0! self-center!">
                    <Checkbox
                      aria-label={word.text}
                      checked={isSelected}
                      onCheckedChange={() => actions.toggle(word.text)}
                      onClick={(event) => event.stopPropagation()}
                    />
                  </ItemMedia>
                )}
                <ItemContent className="min-w-0">
                  <ItemTitle className="block w-full truncate font-normal select-text">
                    {word.text}
                  </ItemTitle>
                </ItemContent>
                {/* With one kind listed, every row is of that kind and need not say so. */}
                {state.filter === "all" && (
                  <SourceBadge
                    source={word.source}
                    label={t(`dictionary.sources.${word.source}`)}
                  />
                )}
                <ItemActions
                  className={cn(
                    "-mr-2 opacity-0 transition-opacity duration-150 group-hover/item:opacity-100 focus-within:opacity-100",
                    isPicking && "invisible",
                  )}
                >
                  {/* A tooltip for every row is costly to build, and no row shows it until it is
                      pointed at. */}
                  <Deferred>
                    <IconButton
                      size="icon-sm"
                      icon={Cancel01Icon}
                      label={t("dictionary.remove", { word: word.text })}
                      onClick={() => actions.remove(word.text)}
                    />
                  </Deferred>
                </ItemActions>
              </Item>
            );
          })}
        </RowList>
      )}

      {/* Room for the bar that floats over the bottom while words are picked. */}
      {isPicking && <div className="h-10" />}

      <MessageDialog
        open={state.draft !== null}
        onDismiss={actions.cancelAdding}
        title={t("dictionary.adding.title")}
        description={t("dictionary.adding.description")}
        actions={
          <>
            <Button variant="outline" onClick={actions.cancelAdding}>
              {t("dictionary.adding.cancel")}
            </Button>
            <Button
              form="add-word"
              type="submit"
              disabled={!state.draft?.trim() || state.isDraftKnown}
            >
              {t("dictionary.add")}
            </Button>
          </>
        }
      >
        <form
          id="add-word"
          className="flex flex-col gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            actions.add();
          }}
        >
          <Input
            autoFocus
            aria-label={t("dictionary.adding.field")}
            placeholder={t("dictionary.adding.field")}
            value={state.draft ?? ""}
            onChange={(event) => actions.type(event.target.value)}
          />
          {state.isDraftKnown && (
            <p className="px-3 text-[13px] text-muted-foreground">{t("dictionary.adding.known")}</p>
          )}
        </form>
      </MessageDialog>

      <MessageDialog
        open={isConfirmingDeleteAll}
        onDismiss={() => setIsConfirmingDeleteAll(false)}
        title={t("dictionary.deleteAll.title")}
        description={t("dictionary.deleteAll.description")}
        actions={
          <>
            <Button variant="outline" onClick={() => setIsConfirmingDeleteAll(false)}>
              {t("dictionary.cancel")}
            </Button>
            <Button
              variant="destructive"
              className={CONFIRM_DESTRUCTIVE}
              onClick={() => {
                actions.removeAll();
                setIsConfirmingDeleteAll(false);
              }}
            >
              {t("dictionary.deleteAll.confirm")}
            </Button>
          </>
        }
      />

      <MessageDialog
        open={isConfirmingDelete}
        onDismiss={() => setIsConfirmingDelete(false)}
        title={t("dictionary.deleteSelected.title", { count: selection?.size ?? 0 })}
        description={t("dictionary.deleteSelected.description")}
        actions={
          <>
            <Button variant="outline" onClick={() => setIsConfirmingDelete(false)}>
              {t("dictionary.cancel")}
            </Button>
            <Button
              variant="destructive"
              className={CONFIRM_DESTRUCTIVE}
              onClick={() => {
                actions.removeSelected();
                setIsConfirmingDelete(false);
              }}
            >
              {t("dictionary.delete")}
            </Button>
          </>
        }
      />
    </Page>
  );
}
