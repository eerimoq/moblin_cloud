/** Where the backend in `backend/` is: `backendUrl` in `config.js`, which every page loads before its own code, `/api` on the same domain unless the deploy replaces it; the dev server proxies `/api` to a local one (see `vite.config.ts`). */
import { links } from "./links";

export const backendUrl = links.api;

export const moblinIconUrl = (icon: string) => `${backendUrl}/icons/${encodeURIComponent(icon)}`;

export class BackendError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

export const isSignedOut = (error: unknown) =>
  error instanceof BackendError && error.status === 401;

export const isForbidden = (error: unknown) =>
  error instanceof BackendError && error.status === 403;

export async function call<T>(path: string, token?: string | null, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers);
  if (token) {
    headers.set("authorization", `Bearer ${token}`);
  }
  const response = await fetch(`${backendUrl}${path}`, { ...init, headers });
  if (!response.ok) {
    throw new BackendError(response.status, await response.text());
  }
  const text = await response.text();
  return text ? JSON.parse(text) : (undefined as T);
}

export const post = (body?: unknown): RequestInit =>
  body === undefined
    ? { method: "POST" }
    : {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      };
