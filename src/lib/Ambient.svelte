<!-- A slow drift of soft colour: warm dusk -> sage -> fog -> and around again. -->
<script lang="ts">
  import { onMount } from "svelte";

  type Palette = { top: string; bottom: string; blobs: string[] };
  const PALETTES: Palette[] = [
    { top: "#3a2f3d", bottom: "#54413f", blobs: ["#8a6a62", "#6c5668", "#a68a72"] }, // warm dusk
    { top: "#2f3b36", bottom: "#44514a", blobs: ["#6f8577", "#5d7470", "#8e9a7e"] }, // sage
    { top: "#363c45", bottom: "#4d535c", blobs: ["#7d8794", "#697587", "#9aa0a6"] }, // fog
  ];
  const PERIOD = 120; // seconds to drift from one palette to the next
  const W = 160, H = 90, FPS = 12; // tiny canvas, stretched by CSS: soft for free, cheap to draw

  const rgb = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  const mix = (a: number[], b: number[], t: number) => a.map((v, i) => v + (b[i] - v) * t);
  const css = (c: number[], alpha = 1) => `rgba(${c.map(Math.round).join()},${alpha})`;
  const parsed = PALETTES.map((p) => ({ top: rgb(p.top), bottom: rgb(p.bottom), blobs: p.blobs.map(rgb) }));

  let canvas: HTMLCanvasElement;

  function draw(seconds: number) {
    const ctx = canvas.getContext("2d")!;
    const phase = (seconds / PERIOD) % parsed.length;
    const from = parsed[Math.floor(phase)], to = parsed[(Math.floor(phase) + 1) % parsed.length];
    const t = phase % 1, ease = t * t * (3 - 2 * t);

    const sky = ctx.createLinearGradient(0, 0, 0, H);
    sky.addColorStop(0, css(mix(from.top, to.top, ease)));
    sky.addColorStop(1, css(mix(from.bottom, to.bottom, ease)));
    ctx.fillStyle = sky;
    ctx.fillRect(0, 0, W, H);

    from.blobs.forEach((c, i) => {
      const colour = mix(c, to.blobs[i], ease);
      const a = seconds / (90 + i * 37) + i * 2.1; // each blob wanders at its own unhurried speed
      const x = W * (0.5 + 0.34 * Math.sin(a)), y = H * (0.5 + 0.3 * Math.cos(a * 0.8 + i));
      const r = W * (0.42 + 0.06 * i);
      const glow = ctx.createRadialGradient(x, y, 0, x, y, r);
      glow.addColorStop(0, css(colour, 0.5));
      glow.addColorStop(1, css(colour, 0));
      ctx.fillStyle = glow;
      ctx.fillRect(0, 0, W, H);
    });
  }

  onMount(() => {
    const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const start = performance.now() - Math.random() * PERIOD * 1000 * parsed.length; // a different mood each visit
    draw((performance.now() - start) / 1000);
    if (still) return;
    const timer = setInterval(() => draw((performance.now() - start) / 1000), 1000 / FPS);
    return () => clearInterval(timer);
  });
</script>

<canvas bind:this={canvas} width={W} height={H}></canvas>
<div class="vignette"></div>

<style>
  canvas, .vignette { position: fixed; inset: 0; width: 100%; height: 100%; }
  .vignette { background: radial-gradient(ellipse at center, transparent 45%, rgba(24, 19, 30, 0.35) 100%); }
</style>
