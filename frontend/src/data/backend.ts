/** Where the backend in `backend/` is: `VITE_BACKEND_URL` when built with it, else `/api` on the same domain; the dev server proxies `/api` to a local one (see `vite.config.ts`). */
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
