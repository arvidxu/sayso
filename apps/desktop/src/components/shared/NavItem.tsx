import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export function NavItem({
  icon,
  label,
  selected = false,
  onClick,
}: {
  icon: IconSvgElement;
  label: string;
  selected?: boolean;
  onClick: () => void;
}) {
  return (
    <Button
      variant="ghost"
      className={cn(
        "nav-item h-10 justify-start gap-2.5 rounded-2xl px-3",
        selected
          ? "nav-item-active font-medium! text-foreground"
          : "text-muted-foreground hover:bg-foreground/5 hover:text-foreground dark:hover:bg-white/5",
      )}
      aria-current={selected ? "page" : undefined}
      onClick={onClick}
    >
      <HugeiconsIcon
        icon={icon}
        strokeWidth={1.5}
        className={cn("size-[18px]", selected && "text-primary")}
      />
      {label}
    </Button>
  );
}
