#!/usr/bin/env python3
"""Build a 50,000-item synthetic library from the free photos already on disk."""
from __future__ import annotations

import os
import random
import shutil
import subprocess
import sys
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageDraw
import piexif

SRC_DIRS = [
    Path("/workspace/photo-app/engine/data/varied"),
    Path("/workspace/photo-app/engine/data/commons"),
]
DEST = Path(sys.argv[1] if len(sys.argv) > 1 else "/tmp/mnemosyne-bench-50k")
N_PHOTOS = 48000
N_SELFIES = 400
N_SHOTS = 1400
N_SHORTS = 120
N_VIDEOS = 60
N_SCREENRECS = 20
TOTAL = N_PHOTOS + N_SELFIES + N_SHOTS + N_SHORTS + N_VIDEOS + N_SCREENRECS  # 50,000

PLACES = [
    # lat, lon, name
    (38.7223, -9.1393),   # Lisbon
    (40.7128, -74.0060),  # NYC
    (34.0522, -118.2437), # LA
    (51.5074, -0.1278),   # London
    (35.6762, 139.6503),  # Tokyo
]

def jpeg_with_exif(img: Image.Image, *, dt: str, make: str, model: str, lens: str | None,
                   lat: float | None, lon: float | None, quality=72) -> bytes:
    img = img.convert("RGB")
    # EXIF thumbnail (the indexer's fast path)
    thumb = img.copy()
    thumb.thumbnail((160, 160))
    tbuf = BytesIO()
    thumb.save(tbuf, format="JPEG", quality=50)
    zeroth = {
        piexif.ImageIFD.Make: make,
        piexif.ImageIFD.Model: model,
        piexif.ImageIFD.DateTime: dt,
        piexif.ImageIFD.Software: "MnemosyneBench",
        piexif.ImageIFD.XResolution: (72, 1),
        piexif.ImageIFD.YResolution: (72, 1),
        piexif.ImageIFD.Orientation: 1,
    }
    exif_ifd = {
        piexif.ExifIFD.DateTimeOriginal: dt,
        piexif.ExifIFD.DateTimeDigitized: dt,
        piexif.ExifIFD.PixelXDimension: img.width,
        piexif.ExifIFD.PixelYDimension: img.height,
        piexif.ExifIFD.OffsetTimeOriginal: b"+00:00",
    }
    if lens:
        exif_ifd[piexif.ExifIFD.LensModel] = lens
    gps = {}
    if lat is not None and lon is not None:
        def dms(v: float, hems: tuple[str, str]):
            ref = hems[0] if v >= 0 else hems[1]
            v = abs(v)
            d = int(v)
            m_f = (v - d) * 60
            m = int(m_f)
            s = round((m_f - m) * 60 * 10000)
            return ref, ((d, 1), (m, 1), (s, 10000))
        lat_ref, lat_dms = dms(lat, ("N", "S"))
        lon_ref, lon_dms = dms(lon, ("E", "W"))
        gps = {
            piexif.GPSIFD.GPSLatitudeRef: lat_ref,
            piexif.GPSIFD.GPSLatitude: lat_dms,
            piexif.GPSIFD.GPSLongitudeRef: lon_ref,
            piexif.GPSIFD.GPSLongitude: lon_dms,
        }
    exif_dict = {"0th": zeroth, "Exif": exif_ifd, "GPS": gps, "1st": {}, "thumbnail": tbuf.getvalue()}
    exif_bytes = piexif.dump(exif_dict)
    out = BytesIO()
    img.save(out, format="JPEG", quality=quality, exif=exif_bytes)
    return out.getvalue()

def load_bases() -> list[Image.Image]:
    files = []
    for d in SRC_DIRS:
        if d.is_dir():
            files.extend(sorted(p for p in d.iterdir() if p.suffix.lower() in {".jpg", ".jpeg", ".png"}))
    bases = []
    for p in files:
        im = Image.open(p).convert("RGB")
        im.thumbnail((640, 640))
        bases.append(im)
    if not bases:
        raise SystemExit("no source photos found")
    return bases

def dt_for(i: int, event: int) -> str:
    # Cluster ~40 photos into an event, then jump a day or more.
    # Dates span 2022-2025.
    day = (event * 3) % 1000  # 0..999
    year = 2022 + (day // 365)
    doy = day % 365
    month = min(12, 1 + doy // 30)
    dom = 1 + (doy % 28)
    hour = 9 + (i % 8)
    minute = (i * 3) % 60
    second = (i * 7) % 60
    return f"{year:04d}:{month:02d}:{dom:02d} {hour:02d}:{minute:02d}:{second:02d}"

def place_for(event: int) -> tuple[float, float]:
    lat, lon = PLACES[event % len(PLACES)]
    # tiny jitter so they stay in one event (<50 km)
    return lat + ((event % 7) - 3) * 0.002, lon + ((event % 5) - 2) * 0.002

def main():
    if DEST.exists():
        shutil.rmtree(DEST)
    photos = DEST / "photos"
    selfies = DEST / "selfies"
    shots = DEST / "screenshots"
    videos = DEST / "videos"
    for d in (photos, selfies, shots, videos):
        d.mkdir(parents=True)

    print("loading bases…", flush=True)
    bases = load_bases()
    print(f"  {len(bases)} source images", flush=True)

    rng = random.Random(42)

    print(f"writing {N_PHOTOS} photos…", flush=True)
    for i in range(N_PHOTOS):
        event = i // 40
        dt = dt_for(i, event)
        lat, lon = place_for(event)
        img = bases[i % len(bases)]
        data = jpeg_with_exif(img, dt=dt, make="Canon", model="EOS R6", lens="RF 24-105mm",
                              lat=lat, lon=lon)
        (photos / f"IMG_{dt_for(i, event).replace(':', '').replace(' ', '_')}_{i:05d}.jpg").write_bytes(data)
        if (i + 1) % 10000 == 0:
            print(f"  {i+1}", flush=True)

    print(f"writing {N_SELFIES} selfies…", flush=True)
    for i in range(N_SELFIES):
        event = 2000 + i // 20
        dt = dt_for(i, event)
        lat, lon = place_for(event)
        img = bases[i % len(bases)]
        data = jpeg_with_exif(img, dt=dt, make="Apple", model="iPhone 14 Pro",
                              lens="iPhone 14 Pro Front Camera", lat=lat, lon=lon)
        (selfies / f"IMG_{i:04d}_selfie.jpg").write_bytes(data)

    print(f"writing {N_SHOTS} screenshots…", flush=True)
    for i in range(N_SHOTS):
        w, h = (1170, 2532) if i % 2 == 0 else (1920, 1080)
        # tiny unique PNGs at the claimed screen size would be huge; store a
        # small unique PNG but set the name + we'll also write real screen-size
        # for a subset. For type tags, filename Screenshot_ is enough.
        # To also exercise screen-resolution detection, make a handful full-size
        # and the rest small with Screenshot names.
        if i < 20:
            im = Image.new("RGB", (w, h), (30 + i, 40, 50))
        else:
            im = Image.new("RGB", (160, 280) if i % 2 == 0 else (160, 90), (20, 30 + (i % 200), 40))
            ImageDraw.Draw(im).text((4, 4), str(i), fill=(255, 255, 255))
        year = 2023 + (i % 3)
        month = 1 + (i % 12)
        day = 1 + (i % 28)
        path = shots / f"Screenshot_{year}-{month:02d}-{day:02d}-{i%24:02d}-{i%60:02d}-{i%60:02d}_{i:04d}.png"
        im.save(path, format="PNG", optimize=True)

    def make_video(path: Path, w: int, h: int, seconds: int, color: str):
        subprocess.check_call([
            "ffmpeg", "-hide_banner", "-loglevel", "error", "-y",
            "-f", "lavfi", "-i", f"color=c={color}:s={w}x{h}:d={seconds}:r=24",
            "-pix_fmt", "yuv420p", "-c:v", "libx264", "-preset", "ultrafast", "-crf", "36",
            str(path),
        ])

    print("writing video templates + copies…", flush=True)
    short_t = DEST / "_short.mp4"
    wide_t = DEST / "_wide.mp4"
    rec_t = DEST / "_rec.mp4"
    make_video(short_t, 320, 568, 4, "blue")
    make_video(wide_t, 640, 360, 20, "green")
    make_video(rec_t, 640, 360, 8, "gray")
    for i in range(N_SHORTS):
        shutil.copyfile(short_t, videos / f"VID_20240601_{i:04d}.mp4")
    for i in range(N_VIDEOS):
        shutil.copyfile(wide_t, videos / f"clip_{i:04d}.mp4")
    for i in range(N_SCREENRECS):
        shutil.copyfile(rec_t, videos / f"Screen_Recording_{i:02d}.mp4")
    short_t.unlink(); wide_t.unlink(); rec_t.unlink()

    n = sum(1 for p in DEST.rglob("*") if p.is_file())
    print(f"done: {n} files in {DEST}", flush=True)
    if n != TOTAL:
        print(f"WARNING: expected {TOTAL}, got {n}", flush=True)
        sys.exit(2)

if __name__ == "__main__":
    main()
