import { useEffect, useMemo, useState } from "react";
import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { Sidebar, type PageId } from "@/components/layout/Sidebar";
import { SettingsPanel } from "@/components/settings/SettingsPanel";
import { UpdateDialog } from "@/components/updates/UpdateDialog";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Dictionary } from "@/pages/Dictionary";
import { History } from "@/pages/History";
import { Home } from "@/pages/Home";
import { onSettingsToggle } from "@/services/window";
import "./App.css";

// The page extends under the title bar, so it has to provide the strip the window is dragged by.
const hasOverlayTitleBar = navigator.userAgent.includes("Mac");

const APP_NAME = "Sayso";

function App() {
  const [page, setPage] = useState<PageId>("home");
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);

  useEffect(() => onSettingsToggle(() => setIsSettingsOpen((isOpen) => !isOpen)), []);

  // Opening the settings re-renders this component, which must not redraw the page behind them.
  const pages = useMemo(
    () => (
      <AnimatePresence mode="wait" initial={false}>
        <motion.div
          key={page}
          className="h-full"
          initial={{ opacity: 0, x: 10 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: -8 }}
          transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
        >
          {page === "home" && <Home />}
          {page === "history" && <History />}
          {page === "dictionary" && <Dictionary />}
        </motion.div>
      </AnimatePresence>
    ),
    [page],
  );

  return (
    <MotionConfig reducedMotion="user">
      <TooltipProvider delay={300}>
        {hasOverlayTitleBar && (
          <div data-tauri-drag-region className="fixed inset-x-0 top-0 z-60 h-7" />
        )}
        <div className="app-shell flex h-dvh min-h-[600px] min-w-[800px]">
          <Sidebar
            name={APP_NAME}
            page={page}
            onSelect={setPage}
            onOpenSettings={() => setIsSettingsOpen(true)}
          />
          <main className="app-canvas relative my-2.5 mr-2.5 min-w-0 flex-1 overflow-hidden rounded-[28px] bg-background">
            {pages}
          </main>
        </div>
        <SettingsPanel open={isSettingsOpen} onOpenChange={setIsSettingsOpen} />
        <UpdateDialog />
      </TooltipProvider>
    </MotionConfig>
  );
}

export default App;
