#!/usr/bin/env python3
"""Screenshots of the built game at 1920x1080, one run through every screen.

    BUILDER_PYTHON=... $BUILDER_PYTHON scripts/shot.py [OUT_DIR]   (default notes/shots)

Chromium runs through tests/browser/engines.py: on the GPU in an invisible Xvfb
window, never a window on the person's screen. It opens the page with
`?fast&net=off&scene`, so the menus keep their hillside, rooms open with no transport and nothing leaves the origin.
Any page or console error, and any off-origin request or socket, fails the run.
"""
import functools
import http.server
import pathlib
import sys
import threading

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests" / "browser"))
import engines  # noqa: E402

SIZE = {"width": 1920, "height": 1080}
TIMEOUT_MS = 15000
# the backdrop's camera glides to the lineup
GLIDE_MS = 3000


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
    }

    def log_message(self, *args):
        pass


def by_id(name):
    return f"[id='{name}']"


def main():
    out = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "notes" / "shots"
    out.mkdir(parents=True, exist_ok=True)
    handler = functools.partial(Handler, directory=str(ROOT / "dist"))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    origin = f"http://127.0.0.1:{server.server_address[1]}"
    url = f"{origin}/index.html?fast&net=off&scene"
    problems = []

    with sync_playwright() as p:
        browser = engines.launch(p, "chromium")
        page = browser.new_page(viewport=SIZE)
        page.set_default_timeout(TIMEOUT_MS)
        page.on("pageerror", lambda e: problems.append(f"page error: {e}"))
        page.on("console", lambda m: problems.append(f"console: {m.text}") if m.type == "error" else None)
        page.on("websocket", lambda ws: problems.append(f"socket: {ws.url}"))
        page.on(
            "request",
            lambda r: None if r.url.startswith((origin + "/", "data:", "blob:")) else problems.append(f"off-origin: {r.url}"),
        )

        def shot(name):
            page.wait_for_timeout(150)
            page.screenshot(path=str(out / f"{name}.png"))
            print("shot", name, "on", page.evaluate("document.body.dataset.screen"))

        def press(name):
            page.click(by_id(name))

        def type_in(name, text):
            page.fill(by_id(name), text)
            page.locator(by_id(name)).blur()

        def fact(what, value):
            print(f"{what}: {value!r}")

        # about:blank first: a goto that differs only by its hash does not
        # reload, and each section starts from a fresh page
        def fresh(hash_part=""):
            page.goto("about:blank")
            page.goto(url + hash_part)
            page.wait_for_selector("body[data-ready='true']", state="attached")
            page.wait_for_selector("body[data-scene='on']", state="attached")
            page.wait_for_timeout(1600)

        def title():
            shot("01-title")

        def settings():
            press("menu.settings")
            page.eval_on_selector(
                by_id("settings.volume"),
                "e => { e.value = '35'; e.dispatchEvent(new Event('input', { bubbles: true })); }",
            )
            page.check(by_id("settings.dog_mode"))
            shot("02-settings-dog-mode")
            page.uncheck(by_id("settings.dog_mode"))
            shot("02-settings")
            fresh()
            press("menu.settings")
            fact("sound after a reload (35 expected)", page.input_value(by_id("settings.volume")))

        def whats_new():
            page.click("#version")
            shot("03-whats-new")

        def solo():
            press("menu.solo")
            page.select_option(by_id("seat.1.role"), "wolf")
            press("seat.2.bot")
            press("seat.3.bot")
            page.select_option(by_id("seat.2.role"), "sheepdog")
            page.select_option(by_id("seat.3.role"), "wolf")
            fact("sheep with two wolves (150 expected)", page.input_value(by_id("lobby.sheep")))
            fact("dock", page.evaluate("document.body.dataset.dock"))
            fact("--dock-w", page.evaluate("document.documentElement.style.getPropertyValue('--dock-w')"))
            page.wait_for_timeout(GLIDE_MS)
            shot("04-solo-lobby")
            press("lobby.start")
            page.wait_for_selector("body[data-capture='on']", state="attached")
            for _ in range(6):
                page.keyboard.press("ArrowRight")
            page.keyboard.down("ArrowRight")
            page.wait_for_timeout(400)
            page.keyboard.up("ArrowRight")
            page.wait_for_timeout(100)
            fact("the toy world after the keys", page.inner_text("#state"))
            shot("05-match-stub")
            press("play.leave")
            fact("after leaving", page.evaluate("document.body.dataset.screen"))

        def online():
            press("menu.online")
            shot("06-online")
            type_in("online.code", "nope")
            press("online.join")
            fact("bad code", page.inner_text("#message"))
            shot("06-online-bad-code")
            press("online.host")
            press("seat.1.bot")
            page.select_option(by_id("seat.0.role"), "sheepdog")
            page.select_option(by_id("seat.1.role"), "wolf")
            type_in("lobby.name", "Morag")
            press("lobby.copy_code")
            fact("copied", page.evaluate("document.body.dataset.copied"))
            press("lobby.copy_link")
            fact("copied", page.evaluate("document.body.dataset.copied"))
            fact("address", page.evaluate("location.hash"))
            page.wait_for_timeout(GLIDE_MS)
            shot("07-online-lobby")
            press("lobby.leave")
            fact("address after leaving", page.evaluate("location.hash"))

        def invited():
            fact("invite link lands on", page.evaluate("document.body.dataset.screen"))
            fact("invite status", page.inner_text("#message"))
            shot("08-joining-from-a-link")

        # a section that fails is reported and the next one still runs
        for section, hash_part in [
            (title, ""),
            (settings, ""),
            (whats_new, ""),
            (solo, ""),
            (online, ""),
            (invited, "#room=ABCDEF"),
        ]:
            try:
                fresh(hash_part)
                section()
            except Exception as e:
                problems.append(f"{section.__name__}: {e}")
        browser.close()
    server.shutdown()

    print("\nproblems:" if problems else "\nno page errors, no off-origin requests")
    for line in problems:
        print(" ", line)
    sys.exit(1 if problems else 0)


if __name__ == "__main__":
    main()
