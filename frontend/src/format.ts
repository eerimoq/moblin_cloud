export const formatBytes = (bytes: number) =>
  bytes >= 1_000_000_000
    ? `${(bytes / 1_000_000_000).toFixed(2)} GB`
    : bytes >= 1_000_000
      ? `${(bytes / 1_000_000).toFixed(1)} MB`
      : bytes >= 1_000
        ? `${(bytes / 1_000).toFixed(0)} kB`
        : `${bytes} B`;
