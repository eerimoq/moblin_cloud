import { describe, expect, test } from "vitest";
import { signInEnabled } from "./frontendConfig";

describe("configured sign-in providers", () => {
  test("hides buttons while availability is unknown", () => {
    expect(signInEnabled(null, "twitch")).toBe(false);
    expect(signInEnabled(null, "kick")).toBe(false);
  });

  test.each([
    [false, false],
    [true, false],
    [false, true],
    [true, true],
  ])("uses backend availability independently: Twitch %s, Kick %s", (twitch, kick) => {
    const providers = { twitch, kick };
    expect(signInEnabled(providers, "twitch")).toBe(twitch);
    expect(signInEnabled(providers, "kick")).toBe(kick);
  });
});
