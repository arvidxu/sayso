// The system, not the interface language, picks a 12-hour or 24-hour clock. Until the native side
// answers, the usual clock of the language is used.
let is24Hour = !new Intl.DateTimeFormat(undefined, { hour: "numeric" }).resolvedOptions().hour12;

const timeFormats = new Map<string, Intl.DateTimeFormat>();

export function setClock(uses12Hours: boolean) {
  is24Hour = !uses12Hours;
}

export function formatTime(time: number, language: string) {
  // Building a formatter is costly and lists format a time per row, so each is built once.
  const key = `${language}/${is24Hour}`;
  let format = timeFormats.get(key);
  if (!format) {
    format = new Intl.DateTimeFormat(language, {
      hour: is24Hour ? "2-digit" : "numeric",
      minute: "2-digit",
      hourCycle: is24Hour ? "h23" : "h12",
    });
    timeFormats.set(key, format);
  }
  return format.format(time);
}

export function formatDate(time: number, language: string) {
  return new Intl.DateTimeFormat(language, { month: "long", day: "numeric" }).format(time);
}

export function formatNumber(value: number, language: string) {
  return new Intl.NumberFormat(language).format(value);
}

export function formatPercent(value: number, language: string, { signed = false } = {}) {
  return new Intl.NumberFormat(language, {
    style: "percent",
    signDisplay: signed ? "exceptZero" : "auto",
  }).format(value);
}

export function formatWeekday(time: number, language: string) {
  return new Intl.DateTimeFormat(language, { weekday: "narrow" }).format(time);
}

// Each amount apart from its unit, so that a page can set the two differently.
export function spanParts(milliseconds: number, language: string) {
  const minutes = Math.round(milliseconds / 60_000);
  const part = (amount: number, unit: "hour" | "minute") => {
    const parts = new Intl.NumberFormat(language, {
      style: "unit",
      unit,
      unitDisplay: "narrow",
    }).formatToParts(amount);
    return {
      amount: parts
        .filter(({ type }) => type !== "unit" && type !== "literal")
        .map(({ value }) => value)
        .join(""),
      unit: parts.find(({ type }) => type === "unit")?.value ?? "",
    };
  };
  const hours = Math.floor(minutes / 60);
  return hours > 0
    ? [part(hours, "hour"), part(minutes % 60, "minute")]
    : [part(minutes, "minute")];
}

export function formatDuration(milliseconds: number) {
  const seconds = Math.round(milliseconds / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}
