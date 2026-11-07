#!/usr/bin/env python3
"""Check the sound page in headless Chromium, one engine and no window.

    python tools/sound/page_check.py

Run it with a Python that has Playwright, after ./scripts/build.sh and
tools/sound/render.py. It serves dist/ on 127.0.0.1 and:

- presses every button on sounds.html with a real click, and fails on a sound
  that does not start, a console error or a Content-Security-Policy problem;
- runs the flock demo and sees the hooves start and stop;
- renders every tune and sound through the page's own build() in an
  OfflineAudioContext and compares it with the WAV in notes/audio/, so
  web/audio.js and tools/sound/render.py stay in step;
- shows the limiter leaving a tune alone, against the node's defaults;
- renders a bark on each side of the listener and measures each ear.

What a pass cannot say: how any of it sounds. Firefox and WebKit are not run.
"""
import functools
import http.server
import math
import pathlib
import struct
import sys
import threading
import wave

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[2]
DIST = ROOT / "dist"
WAVS = ROOT / "notes" / "audio"
# how far the page and the WAV may part, over the loudest quarter of a sound
MOST_DB = 2.0
LEAST_CORRELATION = 0.97


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".json": "application/json",
    }

    def log_message(self, *args):
        pass


RENDER = """
async (ids) => {
  const { build, echo, decodeNoise, makeLimiter } = await import("./audio.js");
  const shim = await import("./pkg/tool_shim.js");
  await shim.default();
  const desk = new shim.SoundDesk(1);
  const setup = JSON.parse(desk.setup()).ok;
  const levels = (buffer) => {
    const a = buffer.getChannelData(0);
    let peak = 0, sum = 0;
    for (let i = 0; i < a.length; i++) {
      peak = Math.max(peak, Math.abs(a[i]));
      sum += a[i] * a[i];
    }
    return [peak, Math.sqrt(sum / a.length)];
  };
  const through = async (program, makeOut) => {
    const seconds = program.length + 1;
    const ctx = new OfflineAudioContext(1, Math.ceil(seconds * 44100), 44100);
    const noise = await decodeNoise(ctx, desk.noise());
    const out = makeOut(ctx);
    out.connect(ctx.destination);
    const bus = ctx.createGain();
    bus.connect(out);
    if (program.echo) echo(ctx, program.echo, bus, out);
    build(ctx, noise, program, bus, 0);
    return ctx.startRendering();
  };
  const plain = (ctx) => ctx.createGain();
  const result = {};
  for (const [kind, id] of ids) {
    const program = JSON.parse(kind === "tune" ? desk.tune(id) : desk.cue(id)).ok;
    const rendered = await through(program, plain);
    const row = { levels: levels(rendered), env: [] };
    const a = rendered.getChannelData(0);
    for (let i = 0; i + 256 <= a.length; i += 256) {
      let sum = 0;
      for (let j = 0; j < 256; j++) sum += a[i + j] * a[i + j];
      row.env.push(Math.sqrt(sum / 256));
    }
    if (kind === "tune") {
      row.limited = levels(await through(program, (ctx) => makeLimiter(ctx, setup.limiter)));
      row.defaults = levels(await through(program, (ctx) => ctx.createDynamicsCompressor()));
    }
    result[id] = row;
  }
  return result;
}
"""

EARS = """
async () => {
  const shim = await import("./pkg/tool_shim.js");
  await shim.default();
  const { build, decodeNoise } = await import("./audio.js");
  const desk = new shim.SoundDesk(1);
  const setup = JSON.parse(desk.setup()).ok;
  const cue = JSON.parse(desk.cue("sheepdog_bark_1")).ok;
  const result = {};
  for (const [name, x, z] of [["right", 10, 0], ["left", -10, 0], ["ahead", 0, -10], ["behind", 0, 10]]) {
    const ctx = new OfflineAudioContext(2, 44100, 44100);
    const noise = await decodeNoise(ctx, desk.noise());
    const panner = ctx.createPanner();
    panner.panningModel = setup.panning;
    panner.rolloffFactor = 0;
    panner.positionX.value = x;
    panner.positionZ.value = z;
    panner.connect(ctx.destination);
    build(ctx, noise, cue, panner, 0.05);
    const rendered = await ctx.startRendering();
    const energy = (a) => a.reduce((sum, v) => sum + v * v, 0);
    result[name] = [energy(rendered.getChannelData(0)), energy(rendered.getChannelData(1))];
  }
  return result;
}
"""

VOLUME = """
async () => {
  const shim = await import("./pkg/tool_shim.js");
  await shim.default();
  const desk = new shim.SoundDesk(1);
  return [desk.master("50"), desk.master(0), desk.master(100)].map((text) => JSON.parse(text).ok);
}
"""


def wav_envelope(path):
    with wave.open(str(path)) as w:
        n = w.getnframes()
        raw = w.readframes(n)
    a = struct.unpack("<" + "h" * n, raw)
    env = []
    for i in range(0, n - 255, 256):
        env.append(math.sqrt(sum((v / 32768) ** 2 for v in a[i : i + 256]) / 256))
    return max(abs(v) for v in a) / 32768, env


def press_everything(page, problems):
    buttons = page.eval_on_selector_all("#lists button", "b => b.map(x => x.dataset.sound)")
    page.locator("#lists button").first.click()
    page.wait_for_selector("body[data-audio='started']", state="attached", timeout=10000)
    # this press comes before the noise is decoded; it must not be lost
    page.wait_for_selector("body[data-last-tune]", state="attached", timeout=5000)
    played = 0
    for k in range(len(buttons)):
        button = page.locator("#lists button").nth(k)
        sound = button.get_attribute("data-sound")
        heading = button.evaluate("x => x.parentElement.querySelector('h2').textContent")
        button.click()
        page.wait_for_timeout(40)
        if heading in ("Sounds", "Loops"):
            if page.get_attribute("body", "data-last-sound") == sound:
                played += 1
            else:
                problems.append(f"{sound} did not start")
    print(f"{len(buttons)} buttons pressed, {played} sounds started")
    return buttons


def run_flock(page):
    page.locator("button[data-sound='400']").click()
    page.wait_for_selector("body[data-audio-voices~='hooves']", state="attached", timeout=8000)
    page.wait_for_timeout(1500)
    page.locator("section:last-child button[data-sound='stop']").click()
    page.wait_for_selector("body[data-audio-voices='']", state="attached", timeout=4000)
    print("400 sheep: the hooves start and stop")


def compare(rendered, problems):
    print(f"{'sound':26} {'page peak':>9} {'WAV peak':>8} {'envelope r':>10} {'most dB':>8}")
    for id_, row in rendered.items():
        wav_peak, env = wav_envelope(WAVS / f"{id_}.wav")
        n = min(len(env), len(row["env"]))
        a, b = row["env"][:n], env[:n]
        mean_a, mean_b = sum(a) / n, sum(b) / n
        cov = sum((x - mean_a) * (y - mean_b) for x, y in zip(a, b))
        var_a = sum((x - mean_a) ** 2 for x in a)
        var_b = sum((y - mean_b) ** 2 for y in b)
        r = cov / math.sqrt(var_a * var_b) if var_a and var_b else 1.0
        loud = sorted(zip(a, b), reverse=True)[: max(1, n // 4)]
        most = max(abs(20 * math.log10(max(x, 1e-6) / max(y, 1e-6))) for x, y in loud)
        print(f"{id_:26} {row['levels'][0]:9.3f} {wav_peak:8.3f} {r:10.4f} {most:8.2f}")
        if r < LEAST_CORRELATION or most > MOST_DB:
            problems.append(f"{id_}: the page and the WAV have parted (r {r:.3f}, {most:.1f} dB)")
        if "limited" in row:
            plain, limited, defaults = row["levels"], row["limited"], row["defaults"]
            print(f"    rms {plain[1]:.3f}, through the limiter {limited[1]:.3f}, at the node's defaults {defaults[1]:.3f}")
            if abs(20 * math.log10(limited[1] / plain[1])) > 1:
                problems.append(f"{id_}: the limiter changes the level")


def check_ears(ears, problems):
    for name, (left, right) in ears.items():
        print(f"{name:7} left ear {left:8.2f}  right ear {right:8.2f}")
    if not ears["right"][1] > ears["right"][0] or not ears["left"][0] > ears["left"][1]:
        problems.append("left and right are swapped")


def main():
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Handler, directory=str(DIST)))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    problems = []
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        page.on("console", lambda m: problems.append("console: " + m.text) if m.type in ("error", "warning") else None)
        page.on("pageerror", lambda e: problems.append("page: " + str(e)))
        page.add_init_script(
            "document.addEventListener('securitypolicyviolation',"
            " e => (window.__csp = window.__csp || []).push(e.violatedDirective + ' ' + e.blockedURI));"
        )
        page.goto(f"http://127.0.0.1:{server.server_address[1]}/sounds.html")
        page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)
        buttons = press_everything(page, problems)
        run_flock(page)
        volume = page.evaluate(VOLUME)
        print("main volume at 50, 0 and 100:", volume)
        if volume != [0.25, 0, 1]:
            problems.append(f"the main volume reads {volume}")
        problems.extend(f"CSP: {c}" for c in page.evaluate("() => window.__csp || []"))
        tunes = {"scotland_the_brave", "flowers_of_the_forest"}
        ids = [["tune", t] for t in sorted(tunes)]
        ids += [["cue", b] for b in buttons if b[0].isalpha() and not b.startswith("stop") and b not in tunes]
        rendered = page.evaluate(RENDER, ids)
        ears = page.evaluate(EARS)
        browser.close()
    server.shutdown()
    compare(rendered, problems)
    check_ears(ears, problems)
    if problems:
        sys.exit("\n".join(["FAILED:"] + problems))
    print("PAGE CHECK PASSED in chromium")


if __name__ == "__main__":
    main()
