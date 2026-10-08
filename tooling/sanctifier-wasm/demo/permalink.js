// Share code through the URL fragment, not a server or a third-party pastebin.
// Fragments are never sent to the origin in HTTP requests. Sharing a URL
// deliberately discloses its embedded source to whoever receives the link.
export const MAX_SHARE_SOURCE_BYTES = 4096;
const PREFIX = "#code=v1.";

export function encodeShareHash(source) {
  if (typeof source !== "string" || !source.trim()) {
    throw new Error("Enter contract source before creating a share link.");
  }
  const bytes = new TextEncoder().encode(source);
  if (bytes.length > MAX_SHARE_SOURCE_BYTES) {
    throw new Error("A share link can hold up to 4 KiB of UTF-8 source. Share larger files separately.");
  }
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return PREFIX + btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

export function decodeShareHash(hash) {
  if (!hash.startsWith("#code=")) return null;
  if (!hash.startsWith(PREFIX)) {
    throw new Error("This share link uses an unsupported format.");
  }
  const encoded = hash.slice(PREFIX.length);
  if (!/^[A-Za-z0-9_-]+$/.test(encoded) || encoded.length > Math.ceil(MAX_SHARE_SOURCE_BYTES / 3) * 4) {
    throw new Error("This share link contains invalid or oversized source.");
  }
  let binary;
  try {
    binary = atob(encoded.replace(/-/g, "+").replace(/_/g, "/"));
  } catch {
    throw new Error("This share link is not valid base64url.");
  }
  if (binary.length > MAX_SHARE_SOURCE_BYTES) {
    throw new Error("The shared source exceeds the 4 KiB limit.");
  }
  try {
    const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
    const source = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    if (!source.trim()) throw new Error("This share link contains empty source.");
    return source;
  } catch (err) {
    if (err instanceof TypeError) throw new Error("This share link contains invalid UTF-8.");
    throw err;
  }
}

export function makeShareUrl(source, currentHref) {
  const target = new URL(currentHref);
  target.hash = encodeShareHash(source);
  return target.href;
}
