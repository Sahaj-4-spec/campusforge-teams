import test from "node:test";
import assert from "node:assert/strict";
import {
  base64ToBytes,
  bytesToBase64,
  createLockerKeyCheck,
  decryptLockerPayload,
  deriveLockerKey,
  encryptLockerPayload,
  randomBytes,
  verifyLockerKey
} from "./lockerCrypto.js";

test("Save Locker encryption round-trips unicode project content", async () => {
  const salt = randomBytes(16);
  const key = await deriveLockerKey("a unique phrase with enough length", salt);
  const original = {
    title: "Research notes 🔐",
    project: "CampusForge",
    content: "Private project notes — only the owner should read this."
  };
  const encrypted = await encryptLockerPayload(key, original);
  assert.deepEqual(await decryptLockerPayload(key, encrypted), original);
  assert.notEqual(encrypted.ciphertext, JSON.stringify(original));
});

test("each item receives a fresh AES-GCM IV", async () => {
  const key = await deriveLockerKey("another secure locker passphrase", randomBytes(16));
  const first = await encryptLockerPayload(key, { title: "Same item" });
  const second = await encryptLockerPayload(key, { title: "Same item" });
  assert.notEqual(first.iv, second.iv);
  assert.notEqual(first.ciphertext, second.ciphertext);
});

test("the encrypted key check accepts the right key and rejects another", async () => {
  const salt = randomBytes(16);
  const ownerKey = await deriveLockerKey("correct locker passphrase", salt);
  const otherKey = await deriveLockerKey("different locker passphrase", salt);
  const verifier = await createLockerKeyCheck(ownerKey, "test-user-id");
  assert.equal(await verifyLockerKey(ownerKey, verifier, "test-user-id"), true);
  assert.equal(await verifyLockerKey(otherKey, verifier, "test-user-id"), false);
  assert.equal(await verifyLockerKey(ownerKey, verifier, "another-user-id"), false);
});

test("base64 codec preserves binary bytes", () => {
  const bytes = new Uint8Array([0, 1, 2, 127, 128, 200, 254, 255]);
  assert.deepEqual(base64ToBytes(bytesToBase64(bytes)), bytes);
});
