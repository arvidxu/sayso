import { useRef, type ReactNode } from "react";
import { Dialog as DialogPrimitive } from "@base-ui/react/dialog";
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { Backdrop } from "@/components/shared/Backdrop";
import { IconButton } from "@/components/shared/IconButton";
import { Dialog, DialogPortal, DialogTitle } from "@/components/ui/dialog";

export function FloatingPanel({
  open,
  onOpenChange,
  title,
  closeLabel,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  closeLabel: string;
  children: ReactNode;
}) {
  // Focus the panel on open, so no control shows a focus ring until the keyboard is used.
  const panel = useRef<HTMLDivElement>(null);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <Backdrop open={open} onClick={() => onOpenChange(false)} className="z-40" />
      <DialogPortal>
        <DialogPrimitive.Popup
          ref={panel}
          initialFocus={panel}
          className="popup settings-window z-50 h-[min(40rem,calc(100%-3rem))] w-[min(56rem,calc(100%-3rem))] overflow-hidden"
        >
          <header className="settings-titlebar flex h-14 shrink-0 items-center justify-between border-b border-foreground/6 pr-3 pl-5">
            <DialogTitle className="text-base font-bold tracking-tight">{title}</DialogTitle>
            <IconButton
              size="icon-sm"
              icon={Cancel01Icon}
              label={closeLabel}
              onClick={() => onOpenChange(false)}
            />
          </header>
          <div className="flex min-h-0 flex-1">{children}</div>
        </DialogPrimitive.Popup>
      </DialogPortal>
    </Dialog>
  );
}
