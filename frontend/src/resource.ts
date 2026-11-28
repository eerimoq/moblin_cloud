import { createEffect, createSignal } from "solid-js";

export type Resource<T> = {
  (): T | undefined;
  readonly latest: T | undefined;
  readonly error: unknown;
};

export function createResource<S, T>(
  source: () => S | undefined | null | false,
  fetcher: (source: S) => T | Promise<T>,
): [Resource<T>] {
  const [value, setValue] = createSignal<T | undefined>(undefined);
  const [error, setError] = createSignal<unknown>(undefined);
  let latest = 0;
  createEffect(source, (current) => {
    const run = ++latest;
    if (current === undefined || current === null || current === false) {
      return;
    }
    setError(undefined);
    Promise.resolve()
      .then(() => fetcher(current))
      .then(
        (fetched) => {
          if (run === latest) {
            setValue(() => fetched);
          }
        },
        (failure) => {
          if (run === latest) {
            setError(() => failure ?? new Error("failed"));
          }
        },
      );
  });
  const resource = (() => value()) as Resource<T>;
  Object.defineProperties(resource, {
    latest: { get: value },
    error: { get: error },
  });
  return [resource];
}
