import { describe, expect, test } from "vitest";
import { signInEnabled } from "./frontendConfig";

describe("sign-in visibility", () => {
  test("preserves upstream defaults for existing config files", () => {
    expect(signInEnabled({ backendUrl: "/api" }, "twitch")).toBe(true);
    expect(signInEnabled({ backendUrl: "/api" }, "kick")).toBe(true);
  });

  test("controls providers independently", () => {
    const config = { backendUrl: "/api", twitchSignInEnabled: true, kickSignInEnabled: false };
    expect(signInEnabled(config, "twitch")).toBe(true);
    expect(signInEnabled(config, "kick")).toBe(false);
    expect(signInEnabled({ ...config, twitchSignInEnabled: false }, "twitch")).toBe(false);
  });
});
