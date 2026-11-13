import { cn } from "@/lib/utils";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { Button } from "@/components/ui/button";
import { DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

// Goes inside a DropdownMenu, before the menu's content.
export function MenuButton({
  icon,
  label,
  size = "icon",
  className,
}: {
  icon: IconSvgElement;
  label: string;
  size?: "icon" | "icon-sm" | "icon-xs";
  className?: string;
}) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <DropdownMenuTrigger
            render={
              <Button
                variant="ghost"
                size={size}
                aria-label={label}
                className={cn("text-muted-foreground", className)}
              />
            }
          />
        }
      >
        <HugeiconsIcon icon={icon} strokeWidth={1.5} />
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
