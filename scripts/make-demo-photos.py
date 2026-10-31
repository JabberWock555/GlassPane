#!/usr/bin/env python3
"""Generate the demo screensaver photos in demo/photos/.

The demo needs pictures that anyone may redistribute, so instead of shipping
someone's photographs we paint simple landscapes from layered noise: a sky
gradient, a sun or moon with glow, a few mountain ridges fading with distance
(aerial perspective) and, for some scenes, a lake that mirrors the sky.

    python3 scripts/make-demo-photos.py          # needs numpy + Pillow

Output is deterministic (fixed seeds), so re-running gives the same files.
"""
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

OUT = Path(__file__).resolve().parent.parent / "demo" / "photos"


def ridge(width, rng, roughness, octaves=7):
    """1-D fractal noise (sum of smoothed random octaves), roughly in [-1, 1]."""
    x = np.linspace(0, 1, width)
    total = np.zeros(width)
    amp, freq = 1.0, 2
    for _ in range(octaves):
        pts = rng.uniform(-1, 1, freq + 1)
        total += amp * np.interp(x, np.linspace(0, 1, freq + 1), pts)
        amp *= roughness
        freq *= 2
    return total / 1.6


def lerp(a, b, t):
    a, b = np.asarray(a, float), np.asarray(b, float)
    return a + (b - a) * np.asarray(t)[..., None]


def scene(name, size, seed, sky, sun, ridges, haze, lake=None, stars=False):
    w, h = size
    rng = np.random.default_rng(seed)
    yy, xx = np.mgrid[0:h, 0:w].astype(float)

    # Sky: vertical gradient through the given colour stops.
    t = yy / h
    stops = np.linspace(0, 1, len(sky))
    img = np.zeros((h, w, 3))
    for i in range(len(sky) - 1):
        m = (t >= stops[i]) & (t <= stops[i + 1])
        local = (t - stops[i]) / (stops[i + 1] - stops[i])
        img[m] = lerp(sky[i], sky[i + 1], local)[m]

    if stars:
        n = int(w * h / 900)
        sx, sy = rng.integers(0, w, n), rng.integers(0, int(h * 0.55), n)
        img[sy, sx] = np.maximum(img[sy, sx], rng.uniform(150, 255, (n, 1)))

    # Sun / moon: a disc plus a wide soft glow.
    cx, cy, r, colour = sun
    d = np.hypot(xx - cx * w, yy - cy * h) / h
    glow = np.exp(-((d / (r * 6)) ** 2)) * 0.55
    img = lerp(img, colour, glow)
    img = lerp(img, (255, 250, 235), np.clip((r - d) * 400, 0, 1))

    # Mountain ridges, far to near; each fades towards the haze colour.
    horizon = lake[0] if lake else 1.0
    for i, (base, height, rough, colour) in enumerate(ridges):
        line = (base - height * ridge(w, rng, rough)) * h
        mask = np.clip(yy - line[None, :], 0, 1.5) / 1.5
        if lake:
            mask = mask * (yy < horizon * h)
        fade = 0.65 * (1 - i / max(len(ridges) - 1, 1))
        tone = lerp(colour, haze, fade)
        # Slight vertical shading inside each ridge for depth.
        shade = np.clip((yy - line[None, :]) / (h * 0.5), 0, 1)[..., None] * 0.25
        img = img * (1 - mask[..., None]) + (tone * (1 - shade)) * mask[..., None]

    # Lake: mirror what's above the waterline, darken, add ripples.
    if lake:
        line, tint = int(lake[0] * h), lake[1]
        below = h - line
        src = img[max(line - below, 0):line][::-1]
        if len(src) < below:
            src = np.concatenate([src, np.repeat(src[-1:], below - len(src), 0)])
        ripple = (np.sin(np.arange(below)[:, None] * 0.9 + xx[:below] * 0.02) * 2).astype(int)
        cols = np.clip(np.arange(w)[None, :] + ripple, 0, w - 1)
        refl = src[np.arange(below)[:, None], cols]
        img[line:] = lerp(refl, tint, 0.35)

    # Film grain keeps gradients from banding after JPEG compression.
    img += rng.normal(0, 2.2, img.shape)
    out = Image.fromarray(np.clip(img, 0, 255).astype(np.uint8))
    out = out.filter(ImageFilter.GaussianBlur(0.6))
    OUT.mkdir(parents=True, exist_ok=True)
    out.save(OUT / f"{name}.jpg", quality=88, optimize=True, progressive=True)
    print("wrote", OUT / f"{name}.jpg", out.size)


L = (2400, 1600)  # landscape, 3:2 like most cameras
P = (1600, 2400)  # portrait

scene("01-alpine-dawn", L, 1,
      sky=[(38, 52, 110), (180, 120, 160), (255, 178, 120), (255, 214, 160)],
      sun=(0.68, 0.62, 0.035, (255, 190, 120)),
      ridges=[(0.60, 0.16, 0.55, (90, 80, 120)), (0.70, 0.14, 0.55, (60, 52, 90)),
              (0.82, 0.10, 0.5, (34, 30, 56)), (0.95, 0.07, 0.45, (16, 14, 28))],
      haze=(230, 160, 150))

scene("02-lake-dusk", L, 7,
      sky=[(20, 24, 60), (90, 60, 120), (240, 120, 90), (255, 190, 120)],
      sun=(0.32, 0.56, 0.03, (255, 150, 90)),
      ridges=[(0.55, 0.12, 0.55, (70, 50, 90)), (0.62, 0.10, 0.5, (40, 30, 60))],
      haze=(220, 120, 110), lake=(0.64, (30, 30, 60)))

scene("03-blue-hour", L, 21,
      sky=[(8, 14, 40), (24, 48, 100), (70, 110, 170), (150, 180, 210)],
      sun=(0.78, 0.22, 0.018, (200, 220, 255)),
      ridges=[(0.62, 0.15, 0.6, (50, 70, 110)), (0.74, 0.12, 0.55, (30, 44, 76)),
              (0.90, 0.08, 0.5, (12, 18, 34))],
      haze=(120, 150, 190), stars=True)

scene("04-misty-valley", L, 42,
      sky=[(170, 196, 210), (210, 222, 220), (236, 228, 210)],
      sun=(0.5, 0.32, 0.025, (255, 240, 210)),
      ridges=[(0.48, 0.10, 0.6, (120, 150, 150)), (0.58, 0.12, 0.6, (90, 120, 120)),
              (0.70, 0.12, 0.55, (60, 90, 88)), (0.84, 0.10, 0.5, (36, 60, 56)),
              (0.97, 0.06, 0.45, (18, 34, 32))],
      haze=(220, 228, 224))

scene("05-desert-night", L, 99,
      sky=[(4, 6, 20), (16, 20, 52), (48, 40, 90), (120, 80, 110)],
      sun=(0.25, 0.25, 0.02, (220, 220, 255)),
      ridges=[(0.80, 0.06, 0.4, (70, 40, 60)), (0.92, 0.04, 0.35, (30, 18, 30))],
      haze=(110, 70, 100), stars=True)

scene("06-peak-portrait", P, 5,
      sky=[(30, 60, 120), (110, 150, 200), (230, 200, 170), (255, 220, 180)],
      sun=(0.55, 0.42, 0.03, (255, 220, 170)),
      ridges=[(0.58, 0.16, 0.5, (90, 100, 140)), (0.72, 0.14, 0.55, (50, 60, 96)),
              (0.90, 0.08, 0.5, (20, 26, 44))],
      haze=(220, 200, 190))
