"""The project's own browser interactions, run by check.py in every engine.

check.py has already loaded the page, waited for it to report ready, and let
it idle. Drive the tool here the way a person would, and assert what the page
shows with `check(condition, "what this shows")`. Assert the effect a person
would see (the text drawn, the file saved), not the markup that should cause it.

Expected values in these checks are worked out by hand from the brief (C4),
never copied from the tool's own output. Wait on a selector or a data-*
attribute the page sets, never on a fixed time: on CI the page may draw with
a CPU and run slowly.
"""

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
STRINGS = json.loads((ROOT / "data" / "strings.json").read_text())
NOTES = json.loads((ROOT / "data" / "patch-notes.json").read_text())


def on_screen(page, name):
    page.wait_for_selector(f"body[data-screen='{name}']", state="attached", timeout=10000)


def run(page, check, engine):
    title(page, check)
    settings(page, check)
    whats_new(page, check)
    solo_lobby(page, check)
    online_lobby(page, check)
    invite_link(page, check)


def by_id(name):
    return f"[id='{name}']"


def offline(page, part=""):
    """The page again with no transport: check.py fails on any WebSocket."""
    url = page.url.split("?")[0] + "?fast&net=off" + part
    page.goto("about:blank")
    page.goto(url)
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)


def title(page, check):
    on_screen(page, "title")
    check(page.inner_text("h1") == STRINGS["title"], "the Title shows the game's name")
    check(page.title() == STRINGS["title"], "the tab shows the game's name")
    for id_, key in (("menu.solo", "menu.solo"), ("menu.online", "menu.online"), ("menu.settings", "menu.settings")):
        check(page.locator(f"[id='{id_}']").text_content() == STRINGS[key], f"the Title offers {STRINGS[key]}")
    check(page.inner_text("#message") == "", "no error message is shown")
    icon = page.get_attribute("#tab-icon", "href")
    check(icon.startswith("data:image/svg+xml,"), "the tab icon is the core's SVG data URL")


def settings(page, check):
    page.click("[id='menu.settings']")
    on_screen(page, "settings")
    # the slider's value is the core's: move it, and read back what is drawn
    page.locator("[id='settings.volume']").fill("35")
    page.wait_for_selector("[id='settings.volume.text']:has-text('35%')", timeout=5000)
    page.locator("[id='settings.dog_mode']").check()
    page.wait_for_selector("body[data-dog='on']", state="attached", timeout=5000)
    page.reload()
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)
    page.click("[id='menu.settings']")
    on_screen(page, "settings")
    check(page.input_value("[id='settings.volume']") == "35", "the sound slider survives a reload")
    check(page.is_checked("[id='settings.dog_mode']"), "Dog mode survives a reload")
    page.locator("[id='settings.dog_mode']").uncheck()
    page.wait_for_selector("body[data-dog='']", state="attached", timeout=5000)
    page.click("[id='menu.back']")
    on_screen(page, "title")


def whats_new(page, check):
    version = NOTES[0]["version"]
    check(page.inner_text("#version") == version, f"the version button says {version}")
    page.click("#version")
    on_screen(page, "notes")
    check(page.inner_text(".note-version") == version, "What's New opens on the newest version")
    check(NOTES[0]["notes"][0] in page.inner_text(".notes"), "What's New shows the version's lines")
    page.click("[id='menu.back']")
    on_screen(page, "title")


def solo_lobby(page, check):
    page.click(by_id("menu.solo"))
    on_screen(page, "lobby")
    # by hand from BRIEF.md's Decisions: two Wolves ask for 150 sheep
    page.select_option(by_id("seat.1.role"), "wolf")
    page.click(by_id("seat.2.bot"))
    page.click(by_id("seat.3.bot"))
    page.select_option(by_id("seat.2.role"), "sheepdog")
    page.select_option(by_id("seat.3.role"), "wolf")
    check(page.input_value(by_id("lobby.sheep")) == "150", "two Wolves bring Number of sheep to 150")
    page.click(by_id("lobby.start"))
    page.wait_for_selector("body[data-capture='on']", state="attached", timeout=10000)
    on_screen(page, "play")
    page.click(by_id("play.leave"))
    on_screen(page, "title")


def online_lobby(page, check):
    offline(page)
    page.click(by_id("menu.online"))
    on_screen(page, "online")
    page.fill(by_id("online.code"), "nope")
    page.locator(by_id("online.code")).blur()
    page.click(by_id("online.join"))
    page.wait_for_selector(f"#message:has-text('{STRINGS['online.bad_code']}')", timeout=5000)
    page.click(by_id("online.host"))
    on_screen(page, "lobby")
    check("#room=" in page.url, "the address carries the room code")
    code = page.url.split("#room=")[1]
    check(code in page.inner_text(by_id("lobby.copy_code.code")), "the lobby shows the room code")
    page.click(by_id("lobby.leave"))
    on_screen(page, "title")
    check("#room=" not in page.url, "leaving clears the room from the address")


def invite_link(page, check):
    offline(page, "#room=ABCDEF")
    on_screen(page, "lobby")
    check(page.inner_text("#message") == STRINGS["online.finding"], "an invite link goes straight to the room")
