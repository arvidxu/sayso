import {
  AudioWave01Icon,
  Book02Icon,
  Clock01Icon,
  Home01Icon,
  LanguageSkillIcon,
  Settings01Icon,
  UserCircleIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useTranslation } from "react-i18next";
import { IconButton } from "@/components/shared/IconButton";
import { MenuButton } from "@/components/shared/MenuButton";
import { NavItem } from "@/components/shared/NavItem";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
} from "@/components/ui/dropdown-menu";
import { useLanguage } from "@/hooks/useLanguage";

export type PageId = "home" | "history" | "dictionary";

const PAGES = [
  { id: "home", icon: Home01Icon },
  { id: "history", icon: Clock01Icon },
  { id: "dictionary", icon: Book02Icon },
] as const;

// The top is left free for the window's buttons.
export function Sidebar({
  name,
  page,
  onSelect,
  onOpenSettings,
}: {
  name: string;
  page: PageId;
  onSelect: (page: PageId) => void;
  onOpenSettings: () => void;
}) {
  const { t } = useTranslation();
  const language = useLanguage();

  return (
    <aside className="app-sidebar flex w-48 shrink-0 flex-col px-3 pt-11 pb-3.5 min-[1100px]:w-52">
      <div className="flex items-center gap-2.5 px-2 pb-6">
        <Avatar className="brand-mark rounded-[15px] after:hidden">
          <AvatarFallback className="rounded-[15px] bg-transparent text-white">
            <HugeiconsIcon icon={AudioWave01Icon} strokeWidth={2.2} className="size-5" />
          </AvatarFallback>
        </Avatar>
        <div className="flex flex-col">
          <span className="text-[19px] leading-5 font-bold tracking-[-0.035em]">{name}</span>
          <span className="mt-1 text-[10px] font-semibold tracking-[0.14em] text-muted-foreground uppercase">
            {t("nav.tagline")}
          </span>
        </div>
      </div>
      <nav aria-label={t("nav.pages")} className="flex flex-col gap-1.5">
        {PAGES.map(({ id, icon }) => (
          <NavItem
            key={id}
            icon={icon}
            label={t(`nav.${id}`)}
            selected={page === id}
            onClick={() => onSelect(id)}
          />
        ))}
      </nav>
      <div className="mt-auto flex items-center justify-between px-1">
        {/* Accounts are not built yet. */}
        <IconButton icon={UserCircleIcon} label={t("nav.account")} disabled />
        <div className="flex gap-0.5">
          <DropdownMenu>
            <MenuButton icon={LanguageSkillIcon} label={t("settings.interfaceLanguage.title")} />
            <DropdownMenuContent side="top" align="end" className="min-w-40">
              <DropdownMenuRadioGroup
                value={language.state.language}
                onValueChange={language.actions.select}
              >
                <DropdownMenuRadioItem value="system">
                  {t("settings.interfaceLanguage.system")}
                </DropdownMenuRadioItem>
                {language.state.languages.map(({ value, label }) => (
                  <DropdownMenuRadioItem key={value} value={value} lang={value}>
                    {label}
                  </DropdownMenuRadioItem>
                ))}
              </DropdownMenuRadioGroup>
            </DropdownMenuContent>
          </DropdownMenu>
          <IconButton icon={Settings01Icon} label={t("nav.settings")} onClick={onOpenSettings} />
        </div>
      </div>
    </aside>
  );
}
