export function formatCapability(value: boolean): string {
  return value ? "available" : "unavailable";
}

export function formatFallbackReason(reason?: string): string {
  if (reason === undefined) {
    return "none";
  }

  return reason.replace(/-/g, " ");
}
