import { createReadStream } from "node:fs";
import { realpath, stat } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_ROOT = resolve(process.cwd(), "client", "dist");
const MIME_TYPES = new Map([
  [".avif", "image/avif"],
  [".css", "text/css; charset=utf-8"],
  [".gif", "image/gif"],
  [".html", "text/html; charset=utf-8"],
  [".ico", "image/x-icon"],
  [".jpeg", "image/jpeg"],
  [".jpg", "image/jpeg"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".map", "application/json; charset=utf-8"],
  [".png", "image/png"],
  [".svg", "image/svg+xml"],
  [".txt", "text/plain; charset=utf-8"],
  [".webp", "image/webp"],
  [".woff", "font/woff"],
  [".woff2", "font/woff2"],
]);

function setSecurityHeaders(response) {
  response.setHeader("X-Content-Type-Options", "nosniff");
  response.setHeader("Referrer-Policy", "strict-origin-when-cross-origin");
  response.setHeader("X-Frame-Options", "DENY");
  response.setHeader("Permissions-Policy", "camera=(), microphone=(), geolocation=()");
}

function isInsideRoot(root, candidate) {
  return candidate === root || candidate.startsWith(root + sep);
}

export function createStaticServer({ rootDir = DEFAULT_ROOT } = {}) {
  const root = resolve(rootDir);
  const rootPath = realpath(root).catch(() => null);

  return createServer(async (request, response) => {
    setSecurityHeaders(response);

    if (request.method !== "GET" && request.method !== "HEAD") {
      response.writeHead(405, { Allow: "GET, HEAD" });
      response.end();
      return;
    }

    let pathname;
    try {
      pathname = decodeURIComponent(new URL(request.url ?? "/", "http://localhost").pathname);
    } catch {
      response.writeHead(400);
      response.end("Bad request");
      return;
    }

    if (pathname.includes("\0") || pathname.includes("\\")) {
      response.writeHead(400);
      response.end("Bad request");
      return;
    }

    // API and WebSocket routes belong to the Rust service. Never return SPA
    // HTML with status 200 for an unavailable/misconfigured API request.
    if (pathname === "/api" || pathname.startsWith("/api/") || pathname === "/ws" || pathname.startsWith("/ws/")) {
      response.writeHead(404, {
        "Content-Type": "application/json; charset=utf-8",
        "Cache-Control": "no-store",
      });
      response.end(JSON.stringify({ error: "CampusForge API is hosted separately; configure VITE_API_BASE_URL." }));
      return;
    }

    const staticRoot = await rootPath;
    if (!staticRoot) {
      response.writeHead(503);
      response.end("Frontend build is not available");
      return;
    }
    const indexPath = await realpath(resolve(staticRoot, "index.html")).catch(() => null);
    const relativePath = pathname.startsWith("/") ? pathname.slice(1) : pathname;
    const candidate = resolve(staticRoot, relativePath || "index.html");
    if (!isInsideRoot(staticRoot, candidate)) {
      response.writeHead(404);
      response.end("Not found");
      return;
    }

    let filePath = candidate;
    let fileInfo;
    try {
      // Resolve symbolic links before stat/open so links cannot expose files
      // elsewhere on the host through the published directory.
      filePath = await realpath(candidate);
      if (!isInsideRoot(staticRoot, filePath)) {
        response.writeHead(404);
        response.end("Not found");
        return;
      }
      fileInfo = await stat(filePath);
      if (!fileInfo.isFile()) fileInfo = null;
    } catch {
      fileInfo = null;
    }

    // Client-side routes such as /login and /signup resolve to the SPA entry.
    // Missing asset-like paths remain 404s rather than receiving index.html.
    if (!fileInfo) {
      if (extname(pathname)) {
        response.writeHead(404);
        response.end("Not found");
        return;
      }
      try {
        if (!indexPath || !isInsideRoot(staticRoot, indexPath)) throw new Error("index.html is unavailable or outside the publish root");
        filePath = indexPath;
        fileInfo = await stat(filePath);
        if (!fileInfo.isFile()) throw new Error("index.html is not a file");
      } catch {
        response.writeHead(503);
        response.end("Frontend build is not available");
        return;
      }
    }

    const extension = extname(filePath).toLowerCase();
    response.writeHead(200, {
      "Content-Type": MIME_TYPES.get(extension) ?? "application/octet-stream",
      "Content-Length": fileInfo.size,
      "Cache-Control": filePath === indexPath ? "no-cache" : "public, max-age=3600",
    });

    if (request.method === "HEAD") {
      response.end();
      return;
    }

    const stream = createReadStream(filePath);
    stream.on("error", () => {
      if (!response.headersSent) response.writeHead(500);
      response.end();
    });
    stream.pipe(response);
  });
}

if (process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1])) {
  const port = Number.parseInt(process.env.PORT ?? "10000", 10);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error("PORT must be a valid TCP port");
  }

  const server = createStaticServer();
  server.listen(port, "0.0.0.0", () => {
    process.stdout.write("CampusForge frontend listening on 0.0.0.0:" + port + "\n");
  });
}
