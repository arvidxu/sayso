import { Children, type ReactElement, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Card } from "@/components/ui/card";
import { ItemGroup, ItemSeparator } from "@/components/ui/item";
import { cn } from "@/lib/utils";

export const ROW = "min-h-12 flex-nowrap rounded-none border-0 px-4 py-2";

const EASE = [0.22, 1, 0.36, 1] as const;

export function RowList({
  title,
  className,
  children,
}: {
  title?: string;
  className?: string;
  children: ReactNode;
}) {
  // Every row gets a key from this, its own or one from its place.
  const rows = Children.toArray(children) as ReactElement[];

  return (
    <motion.section
      layout="position"
      className={cn("flex flex-col gap-2", className)}
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -4 }}
      transition={{ duration: 0.24, ease: EASE }}
    >
      {title && (
        <div className="flex h-5 items-center justify-between gap-3 px-4">
          <h2 className="truncate text-[13px] font-medium text-muted-foreground">{title}</h2>
        </div>
      )}
      <Card size="sm" className="list-card gap-0 py-0">
        {/* The group spaces small rows apart by itself; here the lines do that. */}
        <ItemGroup className="gap-0 has-data-[size=sm]:gap-0">
          <AnimatePresence initial={true}>
            {rows.map((row, index) => (
              // Keyed by the row itself, so a row keeps its own state when one above it goes.
              <motion.div
                key={row.key}
                layout="position"
                className="overflow-hidden"
                initial={{ opacity: 0, y: 4 }}
                animate={{
                  opacity: 1,
                  y: 0,
                  height: "auto",
                  // Only the arrival is staggered; a row that goes folds at once.
                  transition: { duration: 0.22, ease: EASE, delay: Math.min(index, 7) * 0.02 },
                }}
                exit={{ opacity: 0, height: 0 }}
                transition={{ duration: 0.18, ease: EASE }}
              >
                {index > 0 && (
                  <div className="pl-4">
                    <ItemSeparator className="my-0 bg-foreground/6" />
                  </div>
                )}
                {row}
              </motion.div>
            ))}
          </AnimatePresence>
        </ItemGroup>
      </Card>
    </motion.section>
  );
}
