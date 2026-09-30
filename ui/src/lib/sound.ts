/*
 * Short sounds for finished actions, synthesised with Web Audio (no files):
 * a wooden "tok" for success, a lower double knock for failure. Off unless
 * the user turns on Settings → General → Play sounds.
 */

export type Sound = "success" | "error";

let enabled = false;
let context: AudioContext | null = null;

export function setSoundsEnabled(on: boolean) {
  enabled = on;
}

/** A struck wooden block: a sine that drops in pitch and dies away fast. */
function knock(ctx: AudioContext, at: number, freq: number, gain: number) {
  const osc = ctx.createOscillator();
  const env = ctx.createGain();
  osc.type = "sine";
  osc.frequency.setValueAtTime(freq, at);
  osc.frequency.exponentialRampToValueAtTime(freq * 0.62, at + 0.12);
  env.gain.setValueAtTime(0.0001, at);
  env.gain.exponentialRampToValueAtTime(gain, at + 0.006);
  env.gain.exponentialRampToValueAtTime(0.0001, at + 0.18);
  osc.connect(env).connect(ctx.destination);
  osc.start(at);
  osc.stop(at + 0.2);
}

export function play(sound: Sound) {
  if (!enabled || typeof AudioContext === "undefined") return;
  try {
    context ??= new AudioContext();
    const ctx = context;
    if (ctx.state === "suspended") void ctx.resume();
    const now = ctx.currentTime + 0.01;
    if (sound === "success") {
      knock(ctx, now, 880, 0.12);
      knock(ctx, now, 1320, 0.04);
    } else {
      knock(ctx, now, 330, 0.14);
      knock(ctx, now + 0.11, 262, 0.12);
    }
  } catch {
    // No audio device: sounds are a nicety.
  }
}
