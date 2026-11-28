import { mkdtemp, mkdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import { createStaticServer } from "./static-server.mjs";

let tempRoot;
let root;
let server;
let baseUrl;

before(async () => {
  tempRoot = await mkdtemp(join(tmpdir(), "campusforge-static-"));
  root = join(tempRoot, "public");
  await mkdir(join(root, "assets"), { recursive: true });
  await writeFile(join(root, "index.html"), "<!doctype html><title>CampusForge</title>");
  await writeFile(join(root, "assets", "app.js"), "console.log('ok')");
  await writeFile(join(tempRoot, "secret.txt"), "outside-root-sentinel");
  await symlink(join(tempRoot, "secret.txt"), join(root, "linked-secret.txt"));
  server = createStaticServer({ rootDir: root });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  baseUrl = "http://127.0.0.1:" + address.port;
});

after(async () => {
  if (server?.listening) await new Promise((resolve) => server.close(resolve));
  if (tempRoot) await rm(tempRoot, { recursive: true, force: true });
});

test("serves the SPA entry point with non-cacheable HTML", async () => {
  const response = await fetch(baseUrl + "/");
  assert.equal(response.status, 200);
  assert.equal(response.headers.get("content-type"), "text/html; charset=utf-8");
  assert.equal(response.headers.get("cache-control"), "no-cache");
  assert.match(await response.text(), /CampusForge/);
});

test("falls back to index.html for client-side routes", async () => {
  const response = await fetch(baseUrl + "/workspace/board-123");
  assert.equal(response.status, 200);
  assert.match(await response.text(), /CampusForge/);
});

test("serves static assets with the correct MIME type", async () => {
  const response = await fetch(baseUrl + "/assets/app.js");
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type"), /javascript/);
  assert.equal(await response.text(), "console.log('ok')");
});

test("does not return the SPA entry for missing asset paths", async () => {
  const response = await fetch(baseUrl + "/assets/missing.js");
  assert.equal(response.status, 404);
});

test("does not serve files through symlinks that escape the publish root", async () => {
  const response = await fetch(baseUrl + "/linked-secret.txt");
  const body = await response.text();
  assert.equal(response.status, 404);
  assert.equal(body.includes("outside-root-sentinel"), false);
});

test("does not route unavailable API paths to the SPA entry", async () => {
  const response = await fetch(baseUrl + "/api/health");
  assert.equal(response.status, 404);
  assert.match(response.headers.get("content-type"), /application\/json/);
  assert.deepEqual(await response.json(), { error: "CampusForge API is hosted separately; configure VITE_API_BASE_URL." });
});

test("rejects methods other than GET and HEAD", async () => {
  const response = await fetch(baseUrl, { method: "POST" });
  assert.equal(response.status, 405);
  assert.equal(response.headers.get("allow"), "GET, HEAD");
});

test("serves HEAD requests without a response body", async () => {
  const response = await fetch(baseUrl + "/assets/app.js", { method: "HEAD" });
  assert.equal(response.status, 200);
  assert.equal(await response.text(), "");
});

test("rejects encoded traversal to a real file outside the served root", async () => {
  const response = await fetch(baseUrl + "/%2e%2e%2fsecret.txt");
  const body = await response.text();
  assert.equal(response.status, 404);
  assert.equal(body.includes("outside-root-sentinel"), false);
});

test("sets baseline browser security headers", async () => {
  const response = await fetch(baseUrl + "/");
  assert.equal(response.headers.get("x-content-type-options"), "nosniff");
  assert.equal(response.headers.get("x-frame-options"), "DENY");
  assert.equal(response.headers.get("referrer-policy"), "strict-origin-when-cross-origin");
});
