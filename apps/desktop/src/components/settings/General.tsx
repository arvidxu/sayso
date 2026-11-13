import { useTranslation } from "react-i18next";
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
  useComboboxAnchor,
} from "@/components/ui/combobox";
import type { SettingsPanelModel } from "@/hooks/useSettings";
import type { LanguagePreference } from "@/services/language";
import { Group } from "./Group";
import { Row } from "./Row";

// The label is the language's own name. The name is in the interface language, so a language
// can be read and searched either way.
type LanguageOption = { value: LanguagePreference; label: string; name: string };

export function General({ state, actions }: SettingsPanelModel) {
  const { t } = useTranslation();
  const anchor = useComboboxAnchor();
  const languages: LanguageOption[] = [
    {
      value: "system",
      label: t("settings.interfaceLanguage.system"),
      name: t(`settings.interfaceLanguage.names.${state.systemLanguage}`),
    },
    ...state.languages.map((language) => ({
      ...language,
      name: t(`settings.interfaceLanguage.names.${language.value}`),
    })),
  ];

  return (
    <Group title={t("settings.groups.language")}>
      <Row
        title={t("settings.interfaceLanguage.title")}
        description={t("settings.interfaceLanguage.description")}
      >
        <Combobox
          items={languages}
          value={languages.find(({ value }) => value === state.language) ?? languages[0]}
          isItemEqualToValue={(item: LanguageOption, value: LanguageOption) =>
            item.value === value.value
          }
          filter={(language: LanguageOption, query) =>
            [language.label, language.name, language.value].some((text) =>
              text.toLowerCase().includes(query.trim().toLowerCase()),
            )
          }
          onValueChange={(language: LanguageOption | null) => {
            if (language) actions.selectLanguage(language.value);
          }}
          autoHighlight
        >
          {/* The list hangs from this box and lines up with its right edge; it may be wider. */}
          <div ref={anchor} className="shrink-0">
            <ComboboxInput
              aria-label={t("settings.interfaceLanguage.title")}
              className="h-8 w-36"
            />
          </div>
          {/* One radius for the box and the list; the items sit inside it by their padding. */}
          <ComboboxContent anchor={anchor} align="end" className="w-max min-w-(--anchor-width)">
            <ComboboxEmpty>{t("settings.interfaceLanguage.noMatch")}</ComboboxEmpty>
            <ComboboxList>
              {(language: LanguageOption) => (
                <ComboboxItem key={language.value} value={language}>
                  <span className="flex flex-col">
                    <span lang={language.value}>{language.label}</span>
                    <span className="text-xs whitespace-nowrap text-muted-foreground">
                      {language.name}
                    </span>
                  </span>
                </ComboboxItem>
              )}
            </ComboboxList>
          </ComboboxContent>
        </Combobox>
      </Row>
    </Group>
  );
}
