// The "hang" tick: a short soft click synthesized with WebAudio. No audio files.
let ctx: AudioContext | null = null;
let last = 0;

/** The line is hidden: stop the audio clock (the next tick resumes it). */
export function quiet() {
  try {
    void ctx?.suspend();
  } catch {
    // nothing to suspend
  }
}

export function tick(volume = 0.6) {
  if (volume <= 0) return;
  const now = performance.now();
  if (now - last < 60) return; // several cards landing together: one click is enough
  last = now;
  try {
    ctx ??= new AudioContext();
    if (ctx.state === 'suspended') void ctx.resume();
    const t0 = ctx.currentTime;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = 'triangle';
    osc.frequency.setValueAtTime(1500, t0);
    osc.frequency.exponentialRampToValueAtTime(600, t0 + 0.04);
    gain.gain.setValueAtTime(0.0001, t0);
    gain.gain.exponentialRampToValueAtTime(Math.max(0.0002, (0.07 * volume) / 0.6), t0 + 0.004);
    gain.gain.exponentialRampToValueAtTime(0.0001, t0 + 0.06);
    osc.connect(gain).connect(ctx.destination);
    osc.start(t0);
    osc.stop(t0 + 0.07);
  } catch {
    // no audio device: stay silent
  }
}
