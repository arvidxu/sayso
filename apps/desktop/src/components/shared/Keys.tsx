import { Kbd, KbdGroup } from "@/components/ui/kbd";

export function Keys({ keys }: { keys: readonly string[] }) {
  return (
    <KbdGroup className="shrink-0">
      {keys.map((key) => (
        <Kbd
          key={key}
          className="h-7 min-w-8 rounded-md border border-input bg-background px-2 text-foreground shadow-[0_1.5px_0_var(--color-input)]"
        >
          {key}
        </Kbd>
      ))}
    </KbdGroup>
  );
}
