import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelUpdateDownload,
  checkForUpdates,
  downloadUpdate,
  installUpdate,
  onMenuUpdateCheck,
  onUpdateFound,
  postponeUpdate,
  skipUpdate,
  takePendingUpdate,
  UPDATE_CHECK_REQUEST,
  type UpdateOutcome,
} from "@/services/updates";

export type UpdateStep =
  | Exclude<UpdateOutcome, { status: "busy" }>
  | { status: "checking" }
  | { status: "downloading"; version: string };

export function useUpdates() {
  const [step, setStep] = useState<UpdateStep | null>(null);
  const [isInstalling, setIsInstalling] = useState(false);
  // An answer to a check or download the user cancelled is dropped.
  const awaited = useRef<symbol | null>(null);

  const run = useCallback((waiting: UpdateStep, work: () => Promise<UpdateOutcome>) => {
    const id = Symbol();
    awaited.current = id;
    setStep(waiting);
    work()
      .catch((error): UpdateOutcome => ({ status: "failed", message: String(error) }))
      .then((outcome) => {
        if (awaited.current !== id) return;
        awaited.current = null;
        // The other side was busy after all, so nothing was started.
        setStep(outcome.status === "busy" ? null : outcome);
      })
      .catch(console.error);
  }, []);

  // A check asked for during a check or a download changes nothing.
  const check = useCallback(() => {
    if (!awaited.current) run({ status: "checking" }, checkForUpdates);
  }, [run]);

  useEffect(() => {
    // A timed check never interrupts a step the user is in the middle of.
    const offer = (outcome: UpdateOutcome | null) => {
      if (outcome && outcome.status !== "busy") setStep((current) => current ?? outcome);
    };
    addEventListener(UPDATE_CHECK_REQUEST, check);
    const stopMenu = onMenuUpdateCheck(check);
    const stopFound = onUpdateFound(offer);
    // One may have been found before this was listening.
    takePendingUpdate().then(offer, console.error);
    return () => {
      removeEventListener(UPDATE_CHECK_REQUEST, check);
      stopMenu();
      stopFound();
    };
  }, [check]);

  return {
    state: { step, isInstalling },
    actions: {
      download(version: string) {
        run({ status: "downloading", version }, downloadUpdate);
      },
      cancelDownload() {
        awaited.current = null;
        cancelUpdateDownload().catch(console.error);
        setStep(null);
      },
      restart() {
        setIsInstalling(true);
        installUpdate()
          .catch((error): UpdateOutcome => ({ status: "failed", message: String(error) }))
          .then((outcome) => {
            setIsInstalling(false);
            // A check or download was running, so nothing was installed; Restart can be pressed
            // again.
            if (outcome.status !== "busy") setStep(outcome);
          })
          .catch(console.error);
      },
      skip() {
        skipUpdate().catch(console.error);
        setStep(null);
      },
      later() {
        postponeUpdate().catch(console.error);
        setStep(null);
      },
      close() {
        setStep(null);
      },
    },
  };
}
