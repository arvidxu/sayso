import { useRef, type ReactNode } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Loading03Icon } from "@hugeicons/core-free-icons";
import { useTranslation } from "react-i18next";
import { MessageDialog } from "@/components/shared/MessageDialog";
import { Button } from "@/components/ui/button";
import { useUpdates, type UpdateStep } from "@/hooks/useUpdates";
import { ReleaseNotes } from "./ReleaseNotes";

export function UpdateDialog() {
  const { state, actions } = useUpdates();
  const { t } = useTranslation();
  // The dialog keeps showing its last step while it fades out.
  const step = useLastStep(state.step);

  const content = (): {
    title: string;
    description?: string;
    children?: ReactNode;
    actions?: ReactNode;
    onDismiss?: () => void;
  } => {
    const ok = <Button onClick={actions.close}>{t("updates.actions.ok")}</Button>;
    switch (step?.status) {
      case "checking":
        return { title: t("updates.checking.title"), children: <Busy /> };
      case "unavailable":
        return {
          title: t("updates.unavailable.title"),
          description: t("updates.unavailable.description"),
          actions: ok,
          onDismiss: actions.close,
        };
      case "latest":
        return {
          title: t("updates.latest.title"),
          description: t("updates.latest.description", { version: step.version }),
          actions: ok,
          onDismiss: actions.close,
        };
      case "available":
        return {
          title: t("updates.available.title", { version: step.version }),
          description: t("updates.available.description"),
          children: step.notes && <ReleaseNotes notes={step.notes} />,
          actions: (
            <>
              <Button variant="ghost" className="mr-auto" onClick={actions.skip}>
                {t("updates.actions.skip")}
              </Button>
              <Button variant="outline" onClick={actions.later}>
                {t("updates.actions.later")}
              </Button>
              <Button onClick={() => actions.download(step.version)}>
                {t("updates.actions.download")}
              </Button>
            </>
          ),
          // Closing the dialog counts as Later.
          onDismiss: actions.later,
        };
      case "downloading":
        return {
          title: t("updates.downloading.title", { version: step.version }),
          children: <Busy />,
          actions: (
            <Button variant="outline" onClick={actions.cancelDownload}>
              {t("updates.actions.cancel")}
            </Button>
          ),
        };
      case "ready":
        return {
          title: t("updates.ready.title", { version: step.version }),
          description: t("updates.ready.description"),
          children: step.notes && <ReleaseNotes notes={step.notes} />,
          actions: (
            <>
              <Button variant="outline" onClick={actions.close}>
                {t("updates.actions.later")}
              </Button>
              <Button onClick={actions.restart} disabled={state.isInstalling}>
                {t("updates.actions.restart")}
              </Button>
            </>
          ),
          onDismiss: actions.close,
        };
      case "failed":
        return {
          title: t("updates.failed.title"),
          description: t("updates.failed.description"),
          // What went wrong, as reported; it is not translated.
          children: (
            <p className="text-xs break-words text-muted-foreground select-text">{step.message}</p>
          ),
          actions: ok,
          onDismiss: actions.close,
        };
      default:
        return { title: "" };
    }
  };

  return <MessageDialog open={state.step !== null} {...content()} />;
}

function Busy() {
  return (
    <HugeiconsIcon
      icon={Loading03Icon}
      strokeWidth={1.5}
      className="size-5 animate-spin text-muted-foreground"
    />
  );
}

function useLastStep(step: UpdateStep | null) {
  const last = useRef(step);
  if (step) last.current = step;
  return step ?? last.current;
}
