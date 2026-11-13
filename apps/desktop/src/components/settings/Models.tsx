import { useTranslation } from "react-i18next";
import { Group } from "./Group";
import { Row } from "./Row";

// Downloading is not built yet, so the section only says what will be here.
export function Models() {
  const { t } = useTranslation();

  return (
    <Group title={t("settings.groups.speechModels")}>
      <Row
        title={t("settings.models.none.title")}
        description={t("settings.models.none.description")}
      >
        {null}
      </Row>
    </Group>
  );
}
