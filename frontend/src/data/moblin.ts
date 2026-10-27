export const filtersOf = (filters: unknown): Record<string, boolean> => {
  const on: Record<string, boolean> = {};
  if (Array.isArray(filters)) {
    for (let index = 0; index + 1 < filters.length; index += 2) {
      const key = filters[index];
      if (key && typeof key === "object") {
        const name = Object.keys(key)[0];
        if (name) {
          on[name] = filters[index + 1] === true;
        }
      }
    }
  }
  return on;
};

export const formatBits = (bitsPerSecond: number) =>
  bitsPerSecond >= 1_000_000
    ? `${(bitsPerSecond / 1_000_000).toFixed(1)} Mbps`
    : `${Math.round(bitsPerSecond / 1000)} kbps`;
