import { defineConfig } from "vitest/config";

export default defineConfig({
  resolve: { conditions: ["browser"] },
  ssr: { resolve: { conditions: ["browser"] }, noExternal: [/solid-js/, /@solidjs/] },
  test: { include: ["src/**/*.test.ts"], environment: "node", setupFiles: ["src/testSetup.ts"] },
});
