import { type IconSvgElement } from "@hugeicons/react";
import { ComputerIcon, Moon02Icon, Sun03Icon } from "@hugeicons/core-free-icons";
import { useTranslation } from "react-i18next";
import { Filter } from "@/components/shared/Filter";
import type { ThemePreference } from "@/services/theme";
import type { SettingsPanelModel } from "@/hooks/useSettings";
import { Group } from "./Group";
import { Row } from "./Row";

const THEMES: { value: ThemePreference; icon: IconSvgElement }[] = [
  { value: "system", icon: ComputerIcon },
  { value: "light", icon: Sun03Icon },
  { value: "dark", icon: Moon02Icon },
];

export function Appearance({ state, actions }: SettingsPanelModel) {
  const { t } = useTranslation();

  return (
    <Group title={t("settings.groups.theme")}>
      <Row title={t("settings.theme.title")} description={t("settings.theme.description")}>
        <Filter
          aria-label={t("settings.theme.title")}
          value={state.theme}
          onValueChange={actions.selectTheme}
          options={THEMES.map((theme) => ({ ...theme, label: t(`settings.theme.${theme.value}`) }))}
        />
      </Row>
    </Group>
  );
}
