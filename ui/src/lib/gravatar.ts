const cache = new Map<string, Promise<string>>();

/** Gravatar URL for an email. Gravatar accepts SHA-256 hashes of the trimmed, lowercased address. */
export function gravatarUrl(email: string, size: number): Promise<string> {
  const key = `${email.trim().toLowerCase()}|${size}`;
  let url = cache.get(key);
  if (!url) {
    url = sha256Hex(email.trim().toLowerCase()).then(
      // d=404: no image rather than a generic silhouette, so we can show initials.
      (hash) => `https://gravatar.com/avatar/${hash}?s=${size * 2}&d=404`,
    );
    cache.set(key, url);
  }
  return url;
}

async function sha256Hex(text: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("");
}
