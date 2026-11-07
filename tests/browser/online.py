#!/usr/bin/env python3
"""Online check: two browsers meet by pasted codes and play on one origin.

Part of the online kit (kits/online/README.md). Copy it to
tests/browser/online.py, fill in SELECTORS for the project's page, and run it
after scripts/build.sh. It sits beside the browser check, not inside it,
because check.py fails on any WebSocket or off-origin request and must keep
doing so.

How it stays on the origin:

- `?ice=none` makes web/net/config.js drop the STUN servers, so the two
  browsers connect on host candidates alone;
- the pasted-code path carries the whole handshake in the codes, so no relay
  is contacted;
- host addresses are not hidden behind mDNS names, which a CI runner may not
  resolve.

It fails on: a page or console error, any request off the origin, any
WebSocket (the room-code path must not start), a browser that never reaches
the playing state, the two browsers disagreeing at the end, or a browser
reporting a desync.

What it does not cover: the room-code path over public relays, STUN, TURN,
and two real networks. Those are checked by hand before a release and the
result written in HANDOFF.md.

    python3 tests/browser/online.py [chromium firefox ...]   (default: chromium)

The match is the real one now (SECOND-ORDER.md R1.1000): each browser's arrow
key walks its own body. The `state` line reads `mark : mark : tick : checksum`,
each mark being how far that seat has walked and the checksum the newest one
that browser compared. So equal lines at one tick say both browsers ran the same
match, and two marks past zero say each browser's input reached the other.
"""
import functools
import http.server
import json
import pathlib
import sys
import threading

from playwright.sync_api import sync_playwright

import engines

ROOT = pathlib.Path(__file__).resolve().parents[2]
DIST = ROOT / "dist"
STRINGS = json.loads((ROOT / "data" / "strings.json").read_text())

# The project's own element ids. Each one is an element the page draws from
# the core's view; the check reads text and never computes a rule.
SELECTORS = {
    "online": "[id='menu.online']",
    "host": "[id='online.paste_host']",           # starts hosting by pasted code
    "offer_out": "[id='online.offer_out']",       # the host's code, to copy
    "offer_copy": "[id='online.offer_out.copy']",
    "join": "[id='online.paste_join']",           # opens the joiner's form
    "offer_in": "[id='online.offer_in']",         # where the joiner pastes the host's code
    "make_answer": "[id='online.make_answer']",
    "answer_out": "[id='online.answer_out']",     # the joiner's reply code
    "answer_in": "[id='online.answer_in']",       # where the host pastes the reply
    "accept": "[id='online.accept_answer']",
    "status": "#message",                         # shows the core's status string
    "state": "[id='state']",                      # shows a summary of the world to compare
    "joiner_seat": "[id='seat.1']",
    "joiner_role": "[id='seat.1.role']",
    "ready": "[id='lobby.ready']",
    "start": "[id='lobby.start']:not([disabled])",
    "leave": "[id='play.leave']",
}
# A code's box appears once the code is ready. Waiting on a selector works in
# every engine; page.wait_for_function does not run under Firefox when the
# CSP forbids eval.
LOBBY = "body[data-screen='lobby']"
PLAY = "body[data-screen='play']"
HOST_SEES_JOINER = "[id='seat.1']:not([data-look='empty'])"
TIMEOUT_MS = 15000

failures = []


def check(ok, what):
    print(("  PASS " if ok else "  FAIL ") + what)
    if not ok:
        failures.append(what)


class Quiet(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
    }

    def log_message(self, *args):
        pass


def serve():
    handler = functools.partial(Quiet, directory=str(DIST))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


# Both browsers share this machine, so plain local addresses are what lets
# them meet without STUN.
LAUNCH = {
    "chromium": {"args": ["--disable-features=WebRtcHideLocalIpsWithMdns"]},
    "firefox": {"firefox_user_prefs": {"media.peerconnection.ice.obfuscate_host_addresses": False}},
}


def open_page(browser, origin, who, seen):
    context = browser.new_context()
    page = context.new_page()
    page.on("console", lambda m: seen["errors"].append(f"{who}: {m.text}") if m.type == "error" else None)
    page.on("pageerror", lambda e: seen["errors"].append(f"{who}: {e}"))
    page.on("websocket", lambda ws: seen["sockets"].append(f"{who}: {ws.url}"))
    page.on(
        "request",
        lambda r: None if r.url.startswith((origin + "/", "blob:", "data:")) else seen["offsite"].append(r.url),
    )
    page.goto(origin + "/index.html?ice=none&fast&codes")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=TIMEOUT_MS)
    return page


def says(page, key, when):
    """The page's status line reaches the string `key` names."""
    try:
        page.locator(SELECTORS["status"], has_text=STRINGS[key]).wait_for(timeout=TIMEOUT_MS)
        check(True, f"{key} shows for {when}")
    except Exception:
        check(False, f"{key} shows for {when}: {page.inner_text(SELECTORS['status'])!r}")


def run(p, engine, origin):
    print(f"\n[{engine}] online, two browsers, pasted codes")
    browser = engines.launch(p, engine, **LAUNCH.get(engine, {}))
    seen = {"errors": [], "sockets": [], "offsite": []}
    host = open_page(browser, origin, "host", seen)
    joiner = open_page(browser, origin, "joiner", seen)
    s = SELECTORS

    host.click(s["online"])
    host.click(s["host"])
    host.wait_for_selector(s["offer_out"], timeout=TIMEOUT_MS)
    offer = host.input_value(s["offer_out"])
    check(bool(offer), "the host shows a code to send")
    host.click(s["offer_copy"])
    copied = host.get_attribute("body", "data-copied")
    check(copied == offer, "Copy takes the whole invite code")

    joiner.click(s["online"])
    joiner.click(s["join"])

    # A code in the wrong box and a garbled one are refused by name, and the
    # form stays up for another try. These run through the real unpacking in
    # rtc.js, which the core cannot test.
    joiner.fill(s["offer_in"], "A" + offer[1:])
    joiner.click(s["make_answer"])
    says(joiner, "online.wrong_code", "a reply code in the invite box")
    joiner.fill(s["offer_in"], "O" + "garbled" * 8)
    joiner.click(s["make_answer"])
    says(joiner, "online.bad_paste", "a garbled invite code")

    joiner.fill(s["offer_in"], offer)
    joiner.click(s["make_answer"])
    joiner.wait_for_selector(s["answer_out"], timeout=TIMEOUT_MS)
    answer = joiner.input_value(s["answer_out"])
    check(bool(answer), "the joiner shows a reply code")
    says(joiner, "online.send_reply", "the joiner holding a reply")

    host.fill(s["answer_in"], offer)
    host.click(s["accept"])
    says(host, "online.wrong_code", "an invite code in the reply box")

    host.fill(s["answer_in"], answer)
    host.click(s["accept"])
    joiner.wait_for_selector(LOBBY, state="attached", timeout=TIMEOUT_MS)
    check(True, "the joiner hears the host's lobby")
    host.wait_for_selector(HOST_SEES_JOINER, state="attached", timeout=TIMEOUT_MS)
    check(True, "the host seats the joiner")
    host.wait_for_selector(s["offer_out"], timeout=TIMEOUT_MS)
    fresh = host.input_value(s["offer_out"])
    check(bool(fresh) and fresh != offer, "the host has a fresh invite code for the next joiner")

    # Through the host's lobby, over the data channel, and back.
    joiner.select_option(s["joiner_role"], "wolf")
    joiner.click(s["ready"])  # the box follows the host, so not page.check
    host.wait_for_selector(s["joiner_seat"] + "[data-look='wolf'][data-ready='on']", timeout=TIMEOUT_MS)
    check(True, "the joiner's role and Ready reach the host")
    host.click(s["start"], timeout=TIMEOUT_MS)
    for who, page in (("host", host), ("joiner", joiner)):
        page.wait_for_selector(PLAY, state="attached", timeout=TIMEOUT_MS)
        check(True, f"the {who} reaches the match")

    # Play: the project's own inputs go here, the same on both pages.
    # Keep it short; the core suite already plays long lagged matches.
    host.wait_for_selector(s["state"], timeout=TIMEOUT_MS)
    joiner.wait_for_selector(s["state"], timeout=TIMEOUT_MS)
    for _ in range(20):
        host.keyboard.press("ArrowRight")
        joiner.keyboard.press("ArrowLeft")
    host.wait_for_timeout(2000)

    # A checksum goes up every half second, so wait for a moment both browsers
    # show the same tick's.
    for _ in range(200):
        states = [host.inner_text(s["state"]), joiner.inner_text(s["state"])]
        if states[0].split(" : ")[2] == states[1].split(" : ")[2]:
            break
        host.wait_for_timeout(10)
    check(states[0].split(" : ")[2] != "0", f"both browsers compared a checksum {states}")
    check(states[0] == states[1], f"both browsers show the same match {states}")
    markers = states[0].split(" : ")[:2]
    check(markers[0] != "0" and markers[1] != "0", f"each browser's keys moved its own marker {markers}")
    statuses = [host.inner_text(s["status"]), joiner.inner_text(s["status"])]
    check(STRINGS["online.desync"] not in statuses, f"no browser reports a desync {statuses}")
    check(STRINGS["online.garbled"] not in statuses, f"no browser reports a garbled message {statuses}")

    host.click(s["leave"])
    says(joiner, "online.left", "the host leaving")
    check(not seen["errors"], f"no page or console error {seen['errors'][:3]}")
    check(not seen["offsite"], f"no request off the origin {seen['offsite'][:3]}")
    check(not seen["sockets"], f"no WebSocket, so no relay was contacted {seen['sockets'][:3]}")
    browser.close()


def main():
    if not (DIST / "index.html").exists():
        sys.exit("dist/ is missing: run scripts/build.sh first")
    engines = sys.argv[1:] or ["chromium"]
    server = serve()
    origin = "http://127.0.0.1:%d" % server.server_address[1]
    with sync_playwright() as p:
        for engine in engines:
            run(p, engine, origin)
    server.shutdown()
    if failures:
        print(f"\nONLINE CHECK FAILED: {len(failures)} problem(s)")
        for f in failures:
            print("  - " + f)
        sys.exit(1)
    print(f"\nONLINE CHECK PASSED in {', '.join(engines)}")


if __name__ == "__main__":
    main()
