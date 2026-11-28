import type { StoreSetter } from "solid-js";

export type SetStore<T> = {
  <K extends keyof T>(key: K, value: T[K] | ((previous: T[K]) => T[K])): void;
  (values: Partial<T>): void;
};

export function storeSetter<T extends object>(
  update: StoreSetter<T>,
  written?: () => void,
): SetStore<T> {
  return (keyOrValues: keyof T | Partial<T>, value?: unknown) => {
    update((draft) => {
      if (typeof keyOrValues === "object") {
        Object.assign(draft, keyOrValues);
      } else {
        const key = keyOrValues as keyof T;
        draft[key] =
          typeof value === "function"
            ? (value as (previous: T[keyof T]) => T[keyof T])(draft[key])
            : (value as T[keyof T]);
      }
    });
    written?.();
  };
}

export function removeWhere<T>(items: T[], match: (item: T) => boolean) {
  for (let index = items.length - 1; index >= 0; index--) {
    if (match(items[index])) {
      items.splice(index, 1);
    }
  }
}
