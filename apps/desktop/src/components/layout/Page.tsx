import type { ReactNode } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";

const COLUMN = "mx-auto w-full max-w-4xl px-6 min-[1100px]:px-8";

// The scrolling area is as wide as the page, so its scrollbar sits at the page's edge; the
// content keeps to the column.
export function Page({
  title,
  filters,
  search,
  actions,
  floating,
  children,
}: {
  title: string;
  filters?: ReactNode;
  search?: ReactNode;
  actions?: ReactNode;
  floating?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="relative flex h-full flex-col">
      {/* The window is dragged by the top of the page, around its controls. */}
      <header data-tauri-drag-region className="page-header shrink-0 py-3.5">
        <div data-tauri-drag-region className={`${COLUMN} flex h-9 items-center gap-3`}>
          <h1 className="shrink-0 text-[24px] font-bold tracking-[-0.035em]">{title}</h1>
          {filters && <div className="page-filters shrink-0">{filters}</div>}
          <div data-tauri-drag-region className="min-w-0 flex-1 self-stretch" />
          <div className="flex shrink-0 items-center gap-1">
            {search}
            {actions}
          </div>
        </div>
      </header>
      <ScrollArea className="scroll-edge min-h-0 flex-1">
        {/* Room at the end for what floats over it, so the last row can be scrolled clear of it. */}
        <div className={`${COLUMN} flex flex-col gap-6 pt-2 ${floating ? "pb-24" : "pb-10"}`}>
          {children}
        </div>
      </ScrollArea>
      <div className="pointer-events-none absolute inset-x-0 bottom-5 z-20 flex justify-center px-6">
        {floating}
      </div>
    </div>
  );
}
