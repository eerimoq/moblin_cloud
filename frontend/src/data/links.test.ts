import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => vi.unstubAllGlobals());

describe("cloudName", () => {
  it("uses a trimmed instance name only in self-hosted mode", async () => {
    vi.stubGlobal("window", {
      config: { selfHostedLandingPage: true, instanceName: "  My workspace  " },
    });
    const { cloudName } = await import("./links");
    expect(cloudName()).toBe("Moblin Cloud — My workspace");
    window.config.selfHostedLandingPage = false;
    expect(cloudName()).toBe("Moblin Cloud");
  });

  it("keeps the project name when the instance name is absent or blank", async () => {
    vi.stubGlobal("window", { config: { selfHostedLandingPage: true } });
    const { cloudName } = await import("./links");
    expect(cloudName()).toBe("Moblin Cloud");
    window.config.instanceName = "   ";
    expect(cloudName()).toBe("Moblin Cloud");
  });
});
