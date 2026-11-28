import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import { createStaticServer } from "./static-server.mjs";

let root;
let server;
let baseUrl;

before(async () => {
  root = await mkdtemp(join(tmpdir(), "campusforge-static-"));
  await mkdir(join(root, "assets"));
  await writeFile(join(root, "index.html"), "<!doctype html><title>CampusForge</title>");
  await writeFile(join(root, "assets", "app.js"), "console.log('ok')");
  server = createStaticServer({ rootDir: root });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  baseUrl = `http://127.0.0.1:${address.port}`;
});

after(async () => {
  if (server?.listening) await new Promise((resolve) => server.close(resolve));
  if (root) await rm(root, { recursive: true, force: true });
});

test("serves the SPA entry point with non-cacheable HTML", async () => {
  const response = await fetch(baseUrl + "/");
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type"), /text\\/html/);
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

test("sets baseline browser security headers", async () => {
  const response = await fetch(baseUrl + "/");
  assert.equal(response.headers.get("x-content-type-options"), "nosniff");
  assert.equal(response.headers.get("x-frame-options"), "DENY");
  assert.equal(response.headers.get("referrer-policy"), "strict-origin-when-cross-origin");
});
