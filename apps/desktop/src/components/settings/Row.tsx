import type { ReactNode } from "react";
import { ROW } from "@/components/shared/RowList";
import { Item, ItemActions, ItemContent, ItemDescription, ItemTitle } from "@/components/ui/item";

export function Row({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <Item size="sm" className={ROW}>
      <ItemContent className="gap-0.5">
        <ItemTitle className="font-normal">{title}</ItemTitle>
        {description && (
          <ItemDescription className="text-[13px] leading-[18px]">{description}</ItemDescription>
        )}
      </ItemContent>
      <ItemActions>{children}</ItemActions>
    </Item>
  );
}
