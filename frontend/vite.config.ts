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
              `^${base}((dashboard/twitch|twitch|kick)/[^/?.]+|device/[^/?]+(/[^/?]+)?)/?(\\?.*)?$`,
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
        // Every other Twitch and Kick channel, and every one of a streamer
        // without a snapshot, is shown as live, with varying categories and
        // titles and a placeholder thumbnail, whatever the backend says, to see
        // how live channels look without real streams.
        selfHandleResponse: true,
        configure: (proxy) => {
          proxy.on("proxyRes", (proxyRes, req, res) => {
            if (proxyRes.headers["content-type"]?.startsWith("text/event-stream")) {
              res.writeHead(proxyRes.statusCode ?? 500, proxyRes.headers);
              proxyRes.pipe(res);
              return;
            }
            const chunks: Buffer[] = [];
            proxyRes.on("data", (chunk) => chunks.push(chunk));
            proxyRes.on("end", () => {
              let body = Buffer.concat(chunks);
              if (req.url === "/streamers" && proxyRes.statusCode === 200) {
                const { streamers } = JSON.parse(body.toString());
                const counts = { twitch: 0, kick: 0 };
                for (const streamer of streamers) {
                  for (const channel of streamer.channels) {
                    if (channel.platform in counts) {
                      const count = counts[channel.platform as keyof typeof counts]++;
                      channel.live = count % 2 === 0 || streamer.image === null;
                      channel.category = channel.live
                        ? ["Just Chatting", null, "Travel & Outdoors"][count % 3]
                        : null;
                      channel.title =
                        channel.live && count % 4 === 0
                          ? `Walking around town with ${channel.name} 🚶`
                          : null;
                      channel.thumbnail = channel.live
                        ? `https://picsum.photos/seed/${channel.name}/640/360`
                        : null;
                    }
                  }
                }
                body = Buffer.from(JSON.stringify({ streamers }));
              }
              res.writeHead(proxyRes.statusCode ?? 500, {
                ...proxyRes.headers,
                "content-length": body.length,
              });
              res.end(body);
            });
          });
        },
      },
    },
  },
  build: {
    rollupOptions: {
      // One HTML entry per page. Sub pages live in their own directory so
      // GitHub Pages serves them at clean URLs like /chat-bot/.
      input: [
        "index.html",
        "chat-bot/index.html",
        "docs/index.html",
        "cards/index.html",
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
