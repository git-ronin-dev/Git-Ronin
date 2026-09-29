import { useEffect, useState } from "react";

import { gravatarUrl } from "../lib/gravatar";
import { laneColor } from "./colors";

interface AvatarProps {
  name: string;
  email: string;
  size: number;
  /** Fetch from Gravatar; otherwise always show initials. */
  remote: boolean;
}

export function Avatar({ name, email, size, remote }: AvatarProps) {
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!remote) return;
    let live = true;
    void gravatarUrl(email, size).then((u) => live && setUrl(u));
    return () => {
      live = false;
    };
  }, [email, size, remote]);

  const style = { width: size, height: size };
  if (remote && url && !failed) {
    return (
      <img
        src={url}
        alt=""
        style={style}
        className="shrink-0 rounded-full"
        onError={() => setFailed(true)}
      />
    );
  }
  return (
    <span
      aria-hidden
      style={{ ...style, background: laneColor(hash(email)), fontSize: size * 0.45 }}
      className="flex shrink-0 items-center justify-center rounded-full font-semibold text-accent-fg"
    >
      {initials(name)}
    </span>
  );
}

function initials(name: string) {
  const parts = name.trim().split(/\s+/);
  const letters = parts.length > 1 ? parts[0]![0]! + parts.at(-1)![0]! : name.slice(0, 2);
  return letters.toUpperCase();
}

function hash(text: string) {
  let h = 0;
  for (const c of text) h = (h * 31 + c.charCodeAt(0)) | 0;
  return Math.abs(h);
}
