#!/usr/bin/env python3
"""Screenshots and the frame cost of the hillside at 1920x1080, in invisible
GPU Chromium.

    python3 tests/browser/hillside.py SITE OUT name=query [name=query ...]
    python3 tests/browser/hillside.py SITE OUT --cost name=query ...

SITE is the built site (dist/). Each name=query shoots hillside.html?query to
OUT/name.png. With --cost it prints the renderer's name, the grass bake, the
frame forced through the GPU, and the GPU's own time if the browser has
EXT_disjoint_timer_query_webgl2. Nothing opens on the person's screen:
engines.py puts the window on Xvfb.
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
  const f = window.hillsideFrame;
  const gl = f.gl;
  const px = new Uint8Array(4);
  const times = [];
  for (let k = 0; k < frames; k++) {
    const t0 = performance.now();
    f.frame(t0);
    gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
    times.push(performance.now() - t0);
  }
  const parts = [];
  for (let k = 0; k < 8; k++) {
    const t0 = performance.now();
    const view = f.build(t0);
    const t1 = performance.now();
    f.world(view);
    gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
    const t2 = performance.now();
    f.blades(view);
    gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
    parts.push([t1 - t0, t2 - t1, performance.now() - t2]);
  }
  const info = gl.getExtension("WEBGL_debug_renderer_info");
  const name = info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : "unknown";
  const ext = gl.getExtension("EXT_disjoint_timer_query_webgl2");
  const gpu = [];
  if (ext) {
    for (let k = 0; k < 6; k++) {
      const q = gl.createQuery();
      gl.beginQuery(ext.TIME_ELAPSED_EXT, q);
      f.frame(performance.now());
      gl.endQuery(ext.TIME_ELAPSED_EXT);
      gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
      for (let w = 0; w < 200; w++) {
        await new Promise((r) => setTimeout(r, 5));
        if (gl.getQueryParameter(q, gl.QUERY_RESULT_AVAILABLE)) break;
      }
      if (gl.getQueryParameter(q, gl.QUERY_RESULT_AVAILABLE) && !gl.getParameter(ext.GPU_DISJOINT_EXT)) {
        gpu.push(gl.getQueryParameter(q, gl.QUERY_RESULT) / 1e6);
      }
    }
  }
  return { times, gpu, name, parts, timer: !!ext, bake: f.bakeMs };
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
        page.on("console", lambda m: problems.append(m.text) if m.type == "error" and "404" not in m.text else None)
        for job in jobs:
            name, _, query = job.partition("=")
            page.goto(f"http://127.0.0.1:{port}/hillside.html?{query}")
            try:
                page.wait_for_selector('body[data-ready="true"]', state="attached", timeout=90000)
            except Exception as e:
                print(name, "never drew:", problems or e)
                continue
            if cost:
                r = page.evaluate(COST, 40)
                t = sorted(r["times"][8:])
                print(
                    f"{name}: {r['name']}; bake {r['bake']:.0f} ms; "
                    f"frame median {statistics.median(t):.2f} ms, 95th {t[len(t) * 95 // 100]:.2f} ms; "
                    f"timer query {'yes' if r['timer'] else 'no'}"
                    + (f", GPU median {statistics.median(r['gpu']):.2f} ms" if r["gpu"] else "")
                )
                split = [statistics.median(p[i] for p in r["parts"][2:]) for i in range(3)]
                print(f"{name}: core {split[0]:.2f} ms, world and flock {split[1]:.2f} ms, grass {split[2]:.2f} ms")
            else:
                page.wait_for_timeout(300)
                page.screenshot(path=str(out / f"{name}.png"))
                print("shot", name)
            if problems:
                print(name, "problems:", problems[:3])
                problems.clear()
        browser.close()


if __name__ == "__main__":
    main(sys.argv[1:])
