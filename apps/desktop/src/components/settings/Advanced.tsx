import { useState } from "react";
import { useTranslation } from "react-i18next";
import { CONFIRM_DESTRUCTIVE, MessageDialog } from "@/components/shared/MessageDialog";
import { Button } from "@/components/ui/button";
import type { SettingsPanelModel } from "@/hooks/useSettings";
import { Group } from "./Group";
import { Row } from "./Row";

export function Advanced({ actions }: SettingsPanelModel) {
  const { t } = useTranslation();
  const [isConfirmingReset, setIsConfirmingReset] = useState(false);

  return (
    <>
      <Group title={t("settings.groups.reset")}>
        <Row title={t("settings.reset.title")} description={t("settings.reset.description")}>
          <Button variant="outline" size="sm" onClick={() => setIsConfirmingReset(true)}>
            {t("settings.reset.action")}
          </Button>
        </Row>
      </Group>
      <MessageDialog
        open={isConfirmingReset}
        onDismiss={() => setIsConfirmingReset(false)}
        title={t("settings.reset.confirmTitle")}
        description={t("settings.reset.confirmDescription")}
        actions={
          <>
            <Button variant="outline" onClick={() => setIsConfirmingReset(false)}>
              {t("settings.reset.cancel")}
            </Button>
            <Button
              variant="destructive"
              className={CONFIRM_DESTRUCTIVE}
              onClick={() => {
                actions.restoreDefaults();
                setIsConfirmingReset(false);
              }}
            >
              {t("settings.reset.confirm")}
            </Button>
          </>
        }
      />
    </>
  );
}
