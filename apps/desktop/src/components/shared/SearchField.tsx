import { useEffect, useRef, useState } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon, Search01Icon } from "@hugeicons/core-free-icons";
import { IconButton } from "@/components/shared/IconButton";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group";

// The page hides its filters while the field is open, so it never covers another control.
export function SearchField({
  label,
  clearLabel,
  value,
  onChange,
}: {
  label: string;
  clearLabel: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const [isOpen, setIsOpen] = useState(false);
  const holder = useRef<HTMLDivElement>(null);
  // Set when the field is closed from inside it; the button that takes its place then takes the
  // focus, which would otherwise be lost with the field.
  const refocus = useRef(false);
  const close = () => {
    refocus.current = true;
    onChange("");
    setIsOpen(false);
  };
  const isShown = isOpen || value !== "";

  useEffect(() => {
    if (isShown || !refocus.current) return;
    refocus.current = false;
    holder.current?.querySelector("button")?.focus();
  }, [isShown]);

  if (!isShown) {
    return (
      <div ref={holder}>
        <IconButton icon={Search01Icon} label={label} onClick={() => setIsOpen(true)} />
      </div>
    );
  }

  return (
    <div className="search-field-open w-56 shrink-0">
      <InputGroup className="h-9 w-full bg-background/85 shadow-sm">
        <InputGroupAddon>
          <HugeiconsIcon icon={Search01Icon} strokeWidth={1.5} />
        </InputGroupAddon>
        <InputGroupInput
          autoFocus
          aria-label={label}
          placeholder={label}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          onBlur={() => setIsOpen(false)}
          onKeyDown={(event) => {
            if (event.key === "Escape") close();
          }}
        />
        <InputGroupAddon align="inline-end">
          {/* Pressing it must not blur the field first, or the field would fold under the click. */}
          <InputGroupButton
            size="icon-xs"
            aria-label={clearLabel}
            onMouseDown={(event) => event.preventDefault()}
            onClick={close}
          >
            <HugeiconsIcon icon={Cancel01Icon} strokeWidth={1.5} />
          </InputGroupButton>
        </InputGroupAddon>
      </InputGroup>
    </div>
  );
}
