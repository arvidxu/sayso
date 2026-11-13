import type { ReactNode } from "react";
import { RowList } from "@/components/shared/RowList";

export function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <RowList title={title} className="settings-group">
      {children}
    </RowList>
  );
}
