import type { ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { QUICK } from "@/lib/motion";

export function SelectionBar({
  open,
  label,
  children,
}: {
  open: boolean;
  label: string;
  children: ReactNode;
}) {
  return (
    <AnimatePresence>
      {open && (
        <motion.div
          className="pointer-events-auto flex items-center gap-1 rounded-full bg-foreground py-1.5 pr-1.5 pl-5 text-background shadow-xl shadow-black/20"
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 12 }}
          transition={QUICK}
        >
          <span className="pr-3 text-sm whitespace-nowrap tabular-nums">{label}</span>
          {children}
        </motion.div>
      )}
    </AnimatePresence>
  );
}

// Red that reads on the bar in either theme.
export const ON_BAR_DANGER =
  "text-red-300 hover:text-red-300 dark:text-red-600 dark:hover:text-red-600";

// The bar is the inverse of the page.
export const ON_BAR =
  "text-background hover:bg-background/15 hover:text-background dark:hover:bg-background/15";
