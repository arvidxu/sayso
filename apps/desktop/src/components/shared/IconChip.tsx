import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { cn } from "@/lib/utils";

// "glass" is for a chip on a colored surface.
const TONES = {
  blue: "tone-chip-blue text-tone-blue",
  violet: "tone-chip-violet text-tone-violet",
  green: "tone-chip-green text-tone-green",
  pink: "tone-chip-pink text-tone-pink",
  cyan: "tone-chip-cyan text-tone-cyan",
  glass: "bg-white/18 text-white",
};

export type Tone = keyof typeof TONES;

export function IconChip({
  icon,
  tone,
  className,
}: {
  icon: IconSvgElement;
  tone: Tone;
  className?: string;
}) {
  return (
    <Avatar size="lg" className={cn("after:hidden", className)}>
      <AvatarFallback className={TONES[tone]}>
        <HugeiconsIcon icon={icon} strokeWidth={1.8} className="size-5" />
      </AvatarFallback>
    </Avatar>
  );
}
