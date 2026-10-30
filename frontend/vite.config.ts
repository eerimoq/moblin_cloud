import { existsSync } from "node:fs";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import tailwindcss from "@tailwindcss/vite";

// On a custom domain (public/CNAME present) the site lives at the root.
// Until then GitHub Pages serves it from https://eerimoq.github.io/moblin_cloud/.
const base = existsSync("public/CNAME") ? "/" : "/moblin_cloud/";

export default defineConfig({
  base,
  plugins: [
    solid(),
    tailwindcss(),
    {
      name: "twitch-pages",
      configureServer(server) {
        server.middlewares.use((req, res, next) => {
          const host = new URL(`http://${req.headers.host ?? "localhost"}`);
          if (req.url?.startsWith("/api/auth/") && host.hostname !== "localhost") {
            res.writeHead(302, { location: `http://localhost:${host.port}${req.url}` });
            res.end();
            return;
          }
          if (
            req.url &&
            new RegExp(
              `^${base}((dashboard/twitch|twitch|kick)/[^/?.]+|device/[^/?]+(/[^/?]+)?|monitor/[^/?]+)/?(\\?.*)?$`,
            ).test(req.url)
          ) {
            req.url = `${base}404.html`;
          }
          next();
        });
      },
    },
  ],
  server: {
    host: true,
    // Talk to a local backend (`just dev-backend`) during development.
    proxy: {
      "/api": {
        target: "http://127.0.0.1:8080",
        ws: true,
        rewrite: (path) => path.replace(/^\/api/, ""),
      },
    },
  },
  build: {
    rollupOptions: {
      // One HTML entry per page. Sub pages live in their own directory so
      // GitHub Pages serves them at clean URLs like /account/.
      input: [
        "index.html",
        "page/index.html",
        "dashboard/index.html",
        "account/index.html",
        "admin/index.html",
        "terms/index.html",
        "song-requests/index.html",
        "404.html",
      ],
    },
  },
});
