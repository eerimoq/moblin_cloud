import { beforeEach, expect, test, vi } from "vitest";

const { call } = vi.hoisted(() => ({ call: vi.fn() }));
vi.mock("./backend", () => ({ call }));
vi.mock("solid-js", async () => ({
  ...(await vi.importActual("solid-js")),
  onMount: (callback: () => void) => callback(),
}));

beforeEach(() => {
  vi.resetModules();
  call.mockReset();
});

test("shares one availability request across sign-in controls", async () => {
  call.mockResolvedValue({ twitch: true, kick: false });
  const { useSignInProviders } = await import("./signInProviders");
  const first = useSignInProviders();
  const second = useSignInProviders();
  expect(call).toHaveBeenCalledExactlyOnceWith("/auth/providers");
  await vi.waitFor(() => expect(first()).toEqual({ twitch: true, kick: false }));
  expect(second()).toEqual(first());
});

test("does not show providers when availability cannot be loaded", async () => {
  call.mockRejectedValue(new Error("unavailable"));
  const { useSignInProviders } = await import("./signInProviders");
  const providers = useSignInProviders();
  await vi.waitFor(() => expect(providers()).toEqual({ twitch: false, kick: false }));
});
