const encoder = new TextEncoder();
const decoder = new TextDecoder();

export const LOCKER_KDF_ITERATIONS = 600_000;
const LOCKER_CHECK_PREFIX = "campusforge-save-locker-v1:";

function webCrypto() {
  if (!globalThis.crypto?.subtle || !globalThis.crypto?.getRandomValues) {
    throw new Error("This browser does not support the secure encryption features required by Save Locker.");
  }
  return globalThis.crypto;
}

export function randomBytes(length) {
  return webCrypto().getRandomValues(new Uint8Array(length));
}

export function randomUuid() {
  return webCrypto().randomUUID();
}

export function bytesToBase64(input) {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  let binary = "";
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return btoa(binary);
}

export function base64ToBytes(value) {
  if (typeof value !== "string" || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error("Encrypted locker data is invalid.");
  }
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

export async function deriveLockerKey(passphrase, salt) {
  if (typeof passphrase !== "string" || passphrase.length < 12) {
    throw new Error("Use a unique locker passphrase with at least 12 characters.");
  }
  if (!(salt instanceof Uint8Array) || salt.length !== 16) {
    throw new Error("The Save Locker encryption salt is invalid.");
  }
  const crypto = webCrypto();
  const material = await crypto.subtle.importKey(
    "raw",
    encoder.encode(passphrase),
    "PBKDF2",
    false,
    ["deriveKey"]
  );
  return crypto.subtle.deriveKey(
    {
      name: "PBKDF2",
      salt,
      iterations: LOCKER_KDF_ITERATIONS,
      hash: "SHA-256"
    },
    material,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"]
  );
}

export async function encryptLockerPayload(key, payload, context = "") {
  const crypto = webCrypto();
  const iv = randomBytes(12);
  const plaintext = encoder.encode(JSON.stringify(payload));
  const algorithm = { name: "AES-GCM", iv };
  if (context) algorithm.additionalData = encoder.encode(context);
  const ciphertext = await crypto.subtle.encrypt(algorithm, key, plaintext);
  return {
    iv: bytesToBase64(iv),
    ciphertext: bytesToBase64(ciphertext)
  };
}

export async function decryptLockerPayload(key, encrypted, context = "") {
  const crypto = webCrypto();
  const iv = base64ToBytes(encrypted?.iv);
  const ciphertext = base64ToBytes(encrypted?.ciphertext);
  if (iv.length !== 12 || ciphertext.length < 17) {
    throw new Error("Encrypted locker item is incomplete or corrupted.");
  }
  const algorithm = { name: "AES-GCM", iv };
  if (context) algorithm.additionalData = encoder.encode(context);
  const plaintext = await crypto.subtle.decrypt(algorithm, key, ciphertext);
  return JSON.parse(decoder.decode(plaintext));
}

export async function createLockerKeyCheck(key, userId) {
  return encryptLockerPayload(key, { check: LOCKER_CHECK_PREFIX + userId }, "vault-check:" + userId);
}

export async function verifyLockerKey(key, verifier, userId) {
  try {
    const payload = await decryptLockerPayload(key, verifier, "vault-check:" + userId);
    return payload?.check === LOCKER_CHECK_PREFIX + userId;
  } catch {
    return false;
  }
}
