export type AppLocale = "en" | "zh";

const localeSearchParam = "lang";

export function readInitialLocale(): AppLocale {
  if (typeof window === "undefined") {
    return "en";
  }

  const value = new URLSearchParams(window.location.search).get(localeSearchParam);
  return value === "zh" ? "zh" : "en";
}

export function writeLocale(locale: AppLocale) {
  if (typeof window === "undefined") {
    return;
  }

  const url = new URL(window.location.href);
  url.searchParams.set(localeSearchParam, locale);
  window.history.replaceState(null, "", url);
}
