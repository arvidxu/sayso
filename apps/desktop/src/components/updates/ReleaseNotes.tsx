import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowRight01Icon } from "@hugeicons/core-free-icons";
import { useTranslation } from "react-i18next";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { ScrollArea } from "@/components/ui/scroll-area";

export function ReleaseNotes({ notes }: { notes: string }) {
  const { t } = useTranslation();

  return (
    <Collapsible className="flex flex-col gap-2">
      <CollapsibleTrigger className="group flex w-fit items-center gap-1 rounded-md font-medium outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50">
        <HugeiconsIcon
          icon={ArrowRight01Icon}
          strokeWidth={1.5}
          className="size-4 transition-transform duration-150 group-data-panel-open:rotate-90"
        />
        {t("updates.notes")}
      </CollapsibleTrigger>
      <CollapsibleContent className="h-(--collapsible-panel-height) overflow-hidden transition-[height] duration-200 ease-out data-ending-style:h-0 data-starting-style:h-0">
        <ScrollArea className="flex max-h-44 flex-col rounded-lg border bg-foreground/3">
          <p className="p-3 leading-relaxed whitespace-pre-wrap text-muted-foreground select-text">
            {notes}
          </p>
        </ScrollArea>
      </CollapsibleContent>
    </Collapsible>
  );
}
