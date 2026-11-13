import type { IconSvgElement } from "@hugeicons/react";
import {
  AiChipIcon,
  InformationCircleIcon,
  PaintBoardIcon,
  Settings01Icon,
  SlidersHorizontalIcon,
} from "@hugeicons/core-free-icons";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { Deferred } from "@/components/shared/Deferred";
import { FloatingPanel } from "@/components/shared/FloatingPanel";
import { NavItem } from "@/components/shared/NavItem";
import { ScrollArea } from "@/components/ui/scroll-area";
import { QUICK } from "@/lib/motion";
import { useSettings, type SettingsSection } from "@/hooks/useSettings";
import { About } from "./About";
import { Advanced } from "./Advanced";
import { Appearance } from "./Appearance";
import { General } from "./General";
import { Models } from "./Models";

const SECTIONS: { id: SettingsSection; icon: IconSvgElement; Content: typeof General }[] = [
  { id: "general", icon: Settings01Icon, Content: General },
  { id: "appearance", icon: PaintBoardIcon, Content: Appearance },
  { id: "models", icon: AiChipIcon, Content: Models },
  { id: "advanced", icon: SlidersHorizontalIcon, Content: Advanced },
  { id: "about", icon: InformationCircleIcon, Content: About },
];

export function SettingsPanel({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const settings = useSettings();
  const { section } = settings.state;
  const { t } = useTranslation();
  const { Content } = SECTIONS.find(({ id }) => id === section) ?? SECTIONS[0];

  return (
    <FloatingPanel
      open={open}
      onOpenChange={onOpenChange}
      title={t("settings.title")}
      closeLabel={t("settings.close")}
    >
      <nav
        aria-label={t("settings.title")}
        className="settings-sidebar flex w-44 shrink-0 flex-col gap-1.5 border-r border-foreground/6 p-3"
      >
        {SECTIONS.map(({ id, icon }) => (
          <NavItem
            key={id}
            icon={icon}
            label={t(`settings.sections.${id}`)}
            selected={section === id}
            onClick={() => settings.actions.selectSection(id)}
          />
        ))}
      </nav>
      <ScrollArea className="settings-content min-w-0 flex-1">
        <motion.section
          key={section}
          className="p-6 min-[1000px]:p-7"
          initial={{ opacity: 0, y: 4 }}
          animate={{ opacity: 1, y: 0 }}
          transition={QUICK}
        >
          <h2 className="sr-only">{t(`settings.sections.${section}`)}</h2>
          <div className="flex flex-col gap-6">
            {/* The panel pops in first; its controls are built a moment later, while it fades in. */}
            <Deferred>
              <Content {...settings} />
            </Deferred>
          </div>
        </motion.section>
      </ScrollArea>
    </FloatingPanel>
  );
}
