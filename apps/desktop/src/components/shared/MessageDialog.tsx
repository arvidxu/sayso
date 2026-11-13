import { useRef, type ReactNode } from "react";
import { Dialog as DialogPrimitive } from "@base-ui/react/dialog";
import { Backdrop } from "@/components/shared/Backdrop";
import { Dialog, DialogDescription, DialogPortal, DialogTitle } from "@/components/ui/dialog";

export const CONFIRM_DESTRUCTIVE =
  "bg-destructive text-background hover:bg-destructive/90 dark:bg-destructive dark:hover:bg-destructive/90";

// Sits above a floating panel, so it can be shown while one is open.
export function MessageDialog({
  open,
  onDismiss,
  title,
  description,
  children,
  actions,
}: {
  open: boolean;
  // Escape or a click outside; leave it out for a dialog that has to be answered or waited for.
  onDismiss?: () => void;
  title: string;
  description?: string;
  children?: ReactNode;
  actions?: ReactNode;
}) {
  // While the dialog fades out it keeps showing what it showed, whatever the caller has cleared
  // by then; an emptied dialog fading away reads as a flicker.
  const current = { title, description, children, actions };
  const last = useRef(current);
  if (open) last.current = current;
  const shown = open ? current : last.current;

  // Focus the dialog on open, so no button shows a focus ring until the keyboard is used.
  const dialog = useRef<HTMLDivElement>(null);

  return (
    <Dialog
      open={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) onDismiss?.();
      }}
    >
      <Backdrop open={open} onClick={onDismiss} className="z-70" />
      <DialogPortal>
        <DialogPrimitive.Popup
          ref={dialog}
          initialFocus={dialog}
          className="popup z-80 w-[min(26rem,calc(100%-4rem))] gap-4 p-5"
        >
          <div className="flex flex-col gap-1.5">
            <DialogTitle className="line-clamp-3 leading-snug font-semibold">
              {shown.title}
            </DialogTitle>
            {shown.description && <DialogDescription>{shown.description}</DialogDescription>}
          </div>
          {shown.children}
          {shown.actions && <div className="flex flex-wrap justify-end gap-2">{shown.actions}</div>}
        </DialogPrimitive.Popup>
      </DialogPortal>
    </Dialog>
  );
}
