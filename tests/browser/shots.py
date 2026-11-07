#!/usr/bin/env python3
"""Screenshots of the model gallery at 1920x1080, in invisible GPU Chromium.

    python3 tests/browser/shots.py SITE OUT name=query [name=query ...]
    python3 tests/browser/shots.py SITE OUT --cost name=query ...

SITE is the built site (dist/). Each name=query shoots gallery.html?query to
OUT/name.png. With --cost it measures a frame instead: the build and the draw
of many frames, forced through the GPU, and prints milliseconds a frame.
Nothing opens on the person's screen: engines.py puts the window on Xvfb.
"""
import functools
import http.server
import pathlib
import socketserver
import statistics
import sys
import threading

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import engines  # noqa: E402
from playwright.sync_api import sync_playwright  # noqa: E402

COST = """async (frames) => {
  const f = window.galleryFrame;
  const px = new Uint8Array(4);
  const times = [];
  for (let k = 0; k < frames; k++) {
    const t0 = performance.now();
    f.frame(t0);
    // the frame is not done until a pixel can be read back
    f.gl.readPixels(0, 0, 1, 1, f.gl.RGBA, f.gl.UNSIGNED_BYTE, px);
    times.push(performance.now() - t0);
  }
  return times;
}"""


class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass


def serve(site):
    handler = functools.partial(Quiet, directory=str(site))
    server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), handler)
    server.daemon_threads = True
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server.server_address[1]


def main(argv):
    cost = "--cost" in argv
    argv = [a for a in argv if a != "--cost"]
    site, out, jobs = pathlib.Path(argv[0]), pathlib.Path(argv[1]), argv[2:]
    out.mkdir(parents=True, exist_ok=True)
    port = serve(site)
    with sync_playwright() as p:
        browser = engines.launch(p, "chromium")
        page = browser.new_page(viewport={"width": 1920, "height": 1080})
        problems = []
        page.on("pageerror", lambda e: problems.append(str(e)))
        page.on("console", lambda m: problems.append(m.text) if m.type == "error" else None)
        for job in jobs:
            name, query = job.split("=", 1)
            page.goto(f"http://127.0.0.1:{port}/gallery.html?{query}")
            page.wait_for_selector('body[data-ready="true"]', state="attached", timeout=60000)
            if cost:
                page.wait_for_timeout(500)
                times = page.evaluate(COST, 120)[20:]
                print(f"{name}: median {statistics.median(times):.2f} ms, worst {max(times):.2f} ms a frame")
            else:
                page.wait_for_timeout(350)
                page.screenshot(path=str(out / f"{name}.png"))
                print("shot", name)
        print("renderer:", engines.renderer(page))
        browser.close()
    for line in problems[:12]:
        print("PAGE ERROR:", line[:600])
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
