import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { motion } from "motion/react";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";

export function EmptyState({
  icon,
  title,
  description,
}: {
  icon?: IconSvgElement;
  title?: string;
  description: string;
}) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
    >
      <Empty className="min-h-[calc(100dvh-11rem)] gap-3 p-0">
        <EmptyHeader className="gap-1.5">
          {icon && (
            <EmptyMedia variant="default" className="mb-1 text-muted-foreground">
              <HugeiconsIcon icon={icon} strokeWidth={1.5} className="size-7" />
            </EmptyMedia>
          )}
          {title && (
            <EmptyTitle className="text-[15px] font-medium tracking-normal">{title}</EmptyTitle>
          )}
          <EmptyDescription className="text-[13px]">{description}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    </motion.div>
  );
}
