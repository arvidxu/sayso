import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";

export function Filter<Value extends string>({
  value,
  onValueChange,
  options,
  "aria-label": ariaLabel,
}: {
  value: Value;
  onValueChange: (value: Value) => void;
  options: readonly {
    value: Value;
    label: string;
    icon?: IconSvgElement;
  }[];
  "aria-label": string;
}) {
  return (
    <Tabs value={value} onValueChange={(next) => onValueChange(next as Value)}>
      <TabsList aria-label={ariaLabel}>
        {options.map((option) => (
          <TabsTrigger key={option.value} value={option.value} className="px-3">
            {option.icon && (
              <HugeiconsIcon icon={option.icon} strokeWidth={1.5} data-icon="inline-start" />
            )}
            {option.label}
          </TabsTrigger>
        ))}
      </TabsList>
    </Tabs>
  );
}
