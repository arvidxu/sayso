import { HugeiconsIcon } from "@hugeicons/react";
import { RefreshIcon } from "@hugeicons/core-free-icons";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import type { SettingsPanelModel } from "@/hooks/useSettings";
import { Group } from "./Group";
import { Row } from "./Row";

export function About({ state, actions }: SettingsPanelModel) {
  const { t } = useTranslation();

  return (
    <>
      <Group title={t("settings.groups.application")}>
        <Row title={t("settings.version.title")}>
          <span className="text-muted-foreground tabular-nums select-text">{state.version}</span>
        </Row>
      </Group>
      <Group title={t("settings.groups.updates")}>
        <Row
          title={t("settings.updates.automatic.title")}
          description={t("settings.updates.automatic.description")}
        >
          <Switch
            aria-label={t("settings.updates.automatic.title")}
            checked={state.hasAutomaticUpdates}
            onCheckedChange={actions.setAutomaticUpdates}
          />
        </Row>
        <Row title={t("settings.updates.manual.title")}>
          <Button variant="outline" size="sm" onClick={actions.checkForUpdates}>
            <HugeiconsIcon icon={RefreshIcon} strokeWidth={1.5} />
            {t("settings.updates.check")}
          </Button>
        </Row>
      </Group>
    </>
  );
}
