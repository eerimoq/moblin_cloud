import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => vi.unstubAllGlobals());

describe("cloudName", () => {
  it("uses a trimmed instance name on either landing page", async () => {
    vi.stubGlobal("window", {
      config: { landingPage: "compact", instanceName: "  My workspace  " },
    });
    const { cloudName, instanceName } = await import("./links");
    expect(cloudName()).toBe("Moblin Cloud — My workspace");
    expect(instanceName()).toBe("My workspace");
    window.config.landingPage = "default";
    expect(cloudName()).toBe("Moblin Cloud — My workspace");
    expect(instanceName()).toBe("My workspace");
  });

  it("keeps the project name when the instance name is absent or blank", async () => {
    vi.stubGlobal("window", { config: {} });
    const { cloudName } = await import("./links");
    expect(cloudName()).toBe("Moblin Cloud");
    window.config.instanceName = "   ";
    expect(cloudName()).toBe("Moblin Cloud");
  });
});
