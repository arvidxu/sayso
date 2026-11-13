import type { ComponentProps } from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

export function IconButton({
  icon,
  label,
  ...props
}: { icon: IconSvgElement; label: string } & Omit<ComponentProps<typeof Button>, "children">) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            variant="ghost"
            size="icon"
            aria-label={label}
            {...props}
            className={cn("text-muted-foreground", props.className)}
          />
        }
      >
        <HugeiconsIcon icon={icon} strokeWidth={1.5} />
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
