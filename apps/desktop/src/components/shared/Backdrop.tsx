import { useEffect } from "react";
import { AnimatePresence, motion } from "motion/react";
import { BACKDROP_OPACITY, showBackdrop } from "@/lib/backdrop";
import { BACKDROP_FADE } from "@/lib/motion";
import { cn } from "@/lib/utils";

// Drawn here, not by the dialog, whose own backdrop vanishes at once on closing.
export function Backdrop({
  open,
  onClick,
  className,
}: {
  open: boolean;
  onClick?: () => void;
  className?: string;
}) {
  useEffect(() => (open ? showBackdrop() : undefined), [open]);

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          className={cn("fixed inset-0 bg-black", className)}
          onClick={onClick}
          initial={{ opacity: 0 }}
          animate={{ opacity: BACKDROP_OPACITY }}
          exit={{ opacity: 0 }}
          transition={BACKDROP_FADE}
        />
      )}
    </AnimatePresence>
  );
}
