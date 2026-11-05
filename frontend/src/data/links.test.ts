import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => vi.unstubAllGlobals());

describe("policyUrl", () => {
  it("allows absolute web URLs and rejects executable or invalid links", async () => {
    vi.stubGlobal("window", { config: { backendUrl: "/api" } });
    const { policyUrl } = await import("./links");
    expect(policyUrl(" https://example.com/terms ")).toBe("https://example.com/terms");
    expect(policyUrl("http://localhost/privacy")).toBe("http://localhost/privacy");
    for (const value of [
      undefined,
      "",
      "/terms",
      "not a URL",
      "javascript:alert(1)",
      "data:text/html,test",
      "ftp://example.com",
    ]) {
      expect(policyUrl(value)).toBeUndefined();
    }
  });
});

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
