"""Run the public rehearsals with deliberate input in an owned Chrome window."""
import ctypes
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import time
from urllib.parse import quote


WIKIPEDIA = "https://www.wikipedia.org/"
WIKI_ARTICLES = "https://en.wikipedia.org/wiki/"
DEFAULT_QUERY = "Ada Lovelace"
WIKIDATA = "https://www.wikidata.org/wiki/Q7259"
TIME_ZONES = "https://www.iana.org/time-zones"
RELEASES = "https://www.iana.org/time-zones/releases"
CHROME = Path("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
BLANK_TABS = {"about:blank", "chrome://newtab/", "chrome://new-tab-page/"}
MOVE_FRAME_MS = 26
TYPE_DELAY_MS = 90

class RouteChanged(RuntimeError):
    pass


class Point(ctypes.Structure):
    _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]


def ease(progress):
    return progress * progress * (3 - 2 * progress)


def article_url(title):
    return WIKI_ARTICLES + quote(title.replace(" ", "_"), safe="")


def native_input(*arguments):
    helper = os.environ.get("DEMIO_NATIVE_INPUT")
    if not helper:
        raise RuntimeError("Run this rehearsal with the Demio native bundle.")
    result = subprocess.run(
        [helper, "--native-input", *map(str, arguments)], capture_output=True, text=True, timeout=25
    )
    if result.returncode == 3:
        raise RouteChanged(result.stderr.strip())
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or "The Chrome control could not be used.")


class BrowserInput:
    def __init__(self, headless, focus_required):
        self.position = None
        self.focus_required = focus_required
        self.focus_path = None
        self.chrome_pid = None
        self.warp = None
        if not headless and sys.platform == "darwin":
            graphics = ctypes.CDLL("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")
            self.warp = graphics.CGWarpMouseCursorPosition
            self.warp.argtypes = [Point]
            self.warp.restype = ctypes.c_int32
            self.event = graphics.CGEventCreate
            self.event.argtypes = [ctypes.c_void_p]
            self.event.restype = ctypes.c_void_p
            self.location = graphics.CGEventGetLocation
            self.location.argtypes = [ctypes.c_void_p]
            self.location.restype = Point
            foundation = ctypes.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
            self.release = foundation.CFRelease
            self.release.argtypes = [ctypes.c_void_p]
            self.release.restype = None

    def track_focus(self, session, native_controls):
        if not self.focus_required and not native_controls:
            return
        self.focus_path = Path(session) / "foreground-pid"
        children = {}
        processes = subprocess.check_output(["/bin/ps", "-axo", "pid=,ppid=,comm="], text=True)
        for process in processes.splitlines():
            pid, parent, executable = process.split(maxsplit=2)
            children.setdefault(int(parent), []).append((int(pid), Path(executable).name))
        owned = children.get(os.getpid(), []).copy()
        while owned:
            pid, command = owned.pop()
            if command == CHROME.name:
                self.chrome_pid = pid
                return
            owned.extend(children.get(pid, ()))
        raise RuntimeError("Could not identify the owned Chrome process.")

    def verify_focus(self):
        if self.focus_required:
            try:
                focused_pid = int(self.focus_path.read_text())
            except (OSError, ValueError):
                focused_pid = None
            if focused_pid != self.chrome_pid:
                raise RouteChanged("Return to the rehearsal window in Chrome.")

    def move(self, page, x, y):
        self.verify_focus()
        size = page.viewport_size
        start_x, start_y = self.position or (size["width"] * .26, size["height"] * .65)
        origin = None
        if self.warp:
            left, top, frame_width, toolbar, visible = page.evaluate(
                """() => [screenX, screenY, outerWidth - innerWidth,
                          outerHeight - innerHeight, !document.hidden]"""
            )
            if not visible and self.focus_required:
                raise RouteChanged("Return to the rehearsal tab in Chrome.")
            if visible and 0 <= frame_width <= 12 and 0 < toolbar < 250:
                origin = (left + frame_width / 2, top + toolbar - 1)
                event = self.event(None)
                if event:
                    cursor = self.location(event)
                    self.release(event)
                    start_x, start_y = cursor.x - origin[0], cursor.y - origin[1]
        for step in range(19):
            if step in (6, 12, 18):
                self.verify_focus()
            eased = ease(step / 18)
            px = start_x + (x - start_x) * eased
            py = start_y + (y - start_y) * eased
            if origin:
                self.warp(Point(origin[0] + px, origin[1] + py))
            if 0 <= px < size["width"] and 0 <= py < size["height"]:
                page.mouse.move(px, py)
            page.wait_for_timeout(MOVE_FRAME_MS)
        self.position = (x, y)

    def click(self, page, target, modifier=None):
        target.wait_for(state="visible")
        size = page.viewport_size
        for _ in range(16):
            box = target.bounding_box()
            if not box:
                raise RuntimeError("The expected control lost its visible position.")
            x = box["x"] + box["width"] / 2
            y = box["y"] + box["height"] / 2
            if not 0 < x < size["width"]:
                raise RuntimeError("The expected control is outside the browser's width.")
            if 0 < y < size["height"]:
                break
            # Bring the target into view with real wheel input.
            delta = y - size["height"] / 2
            self.wheel(page, max(-560, min(560, round(delta))))
        else:
            raise RuntimeError("The expected control did not scroll into view.")
        self.move(page, x, y)
        if not target.evaluate(
            "(element, [x, y]) => element.contains(document.elementFromPoint(x, y))",
            [x, y],
        ):
            raise RuntimeError("Another element covers the expected control.")
        if modifier:
            page.keyboard.down(modifier)
        try:
            page.mouse.click(x, y, delay=100)
        finally:
            if modifier:
                page.keyboard.up(modifier)
        page.wait_for_timeout(280)

    def wheel(self, page, distance):
        size = page.viewport_size
        self.move(page, size["width"] * .58, size["height"] * .68)
        before = page.evaluate("scrollY")
        sent = 0
        frames = max(16, min(48, abs(distance) // 20))
        for frame in range(1, frames + 1):
            traveled = round(distance * ease(frame / frames))
            if traveled != sent:
                page.mouse.wheel(0, traveled - sent)
                sent = traveled
            page.wait_for_timeout(29)
        page.wait_for_timeout(80)
        if page.evaluate("scrollY") == before:
            raise RuntimeError("The browser view did not scroll.")


class Walkthrough:
    def __init__(self):
        self.parameters = json.loads(os.environ.get("DEMIO_PARAMETERS", "{}"))
        self.route = self.parameters["route"]
        self.visible_chrome = self.parameters.get("visible_chrome", False)
        # The section and Wikidata routes follow Ada Lovelace's original pages.
        self.query = self.parameters.get("query", DEFAULT_QUERY) if self.route == "article" else DEFAULT_QUERY
        self.article = article_url(self.query)
        self.link_title = self.parameters.get("link_title")
        self.link_url = article_url(self.link_title) if self.link_title else None
        self.section_fragment = "#" + quote(self.parameters["section"], safe="_") if "section" in self.parameters else ""
        self.view_names = {
            "about:blank": "the blank Chrome tab",
            WIKIPEDIA: "Wikipedia search",
            self.article: f"the {self.query} article",
            self.article + "#Biography": "Biography in Wikipedia",
            self.article + "#Work": "Work in Wikipedia",
            WIKIDATA: "the Ada Lovelace entry in Wikidata",
            TIME_ZONES: "IANA Time Zones",
            RELEASES: "IANA Release History",
        }
        if self.link_url:
            self.view_names[self.link_url] = f"the {self.link_title} article"
        if self.section_fragment:
            self.view_names[self.article + self.section_fragment] = f"the section in {self.query}'s article"
        self.headless = os.environ.get("DEMIO_HEADLESS") == "1"
        self.native_chrome = self.visible_chrome and not self.headless
        self.focus_required = not self.headless and os.environ.get("DEMIO_TERMINAL_CONTROLS") != "1"
        self.input = BrowserInput(self.headless, self.focus_required)
        self.playwright = self.context = self.page = self.data_tab = None

    def check(self):
        from playwright.sync_api import sync_playwright

        if self.headless:
            with sync_playwright() as probe:
                if not Path(probe.chromium.executable_path).is_file():
                    raise RuntimeError("Install pinned Chromium with Playwright before running.")
        elif not CHROME.is_file():
            raise RuntimeError("Install Google Chrome in /Applications before running.")
        if self.native_chrome:
            native_input("check")

    def verify_page(self, page, url, heading):
        from playwright.sync_api import expect

        page.wait_for_url(url, wait_until="domcontentloaded", timeout=25000)
        expect(page.locator("h1")).to_contain_text(heading, timeout=10000)

    def prepare(self, session):
        from playwright.sync_api import sync_playwright

        if self.native_chrome:
            native_input("check")
        self.playwright = sync_playwright().start()
        self.context = self.playwright.chromium.launch_persistent_context(
            str(Path(session) / "browser"),
            channel=None if self.headless else "chrome",
            headless=self.headless,
            viewport={"width": 1280, "height": 800},
            locale="en-US",
            color_scheme="light",
            args=["--lang=en-US"] if self.native_chrome else None,
        )
        self.context.set_default_timeout(10000)
        self.input.track_focus(session, self.native_chrome)
        self.page = self.context.pages[0]
        if not self.visible_chrome:
            self.open_site()
        self.page.bring_to_front()

    def open_site(self, show_input=False):
        landing = TIME_ZONES if self.route == "download" else WIKIPEDIA
        if show_input and self.native_chrome:
            native_input("address", self.input.chrome_pid, landing)
            self.page.wait_for_url(landing, wait_until="domcontentloaded", timeout=25000)
        else:
            response = self.page.goto(landing, wait_until="domcontentloaded", timeout=25000)
            if not response or not response.ok:
                raise RuntimeError(f"The public landing page did not load: {landing}")
        if self.route == "download":
            self.verify_page(self.page, TIME_ZONES, "Time Zones")
        else:
            self.page.locator("#searchInput").wait_for(state="visible")

    def inspect_route(self, step):
        self.input.verify_focus()
        url = {
            "search": WIKIPEDIA, "open-article": WIKIPEDIA,
            "article": self.article, "biography": self.article, "read-article": self.article,
            "follow-article": self.article, "read-section": self.article,
            "data": self.article + "#Work", "return": WIKIDATA,
            "await-work": self.article + "#Biography", "check-work": self.article + "#Biography",
            "read-work": self.article + "#Work",
        }.get(step)
        page = self.page
        tabs = [self.page]
        if step == "open-site":
            url = "about:blank"
        elif self.route == "download":
            release = RELEASES + "/" + self.parameters["release"]
            url = {
                "release-history": TIME_ZONES, "release": RELEASES,
                "timer": release, "download": release, "return": release, "close": RELEASES,
            }.get(step)
        elif self.route == "navigation":
            if step == "close" and self.page.is_closed():
                # The first click may already have closed Wikipedia in this step.
                page, tabs, url = self.data_tab, [self.data_tab], WIKIDATA
            elif step == "return":
                page = self.data_tab
                tabs.append(self.data_tab)
            elif step == "close":
                tabs.append(self.data_tab)
                url = self.article + "#Biography"
        elif step == "close":
            url = (
                self.link_url or self.article + self.section_fragment
                if self.route == "article" else self.article + "#Work"
            )
        if url is None:
            raise RuntimeError(f"No route defined for {step}.")
        if not self.context or not page or page.is_closed():
            raise RouteChanged("The rehearsal tab was closed. Stop and start a fresh run.")
        try:
            current, focused, visible, query, result = page.evaluate(
                """([search, article]) => {
                    const input = search && document.getElementById('searchInput');
                    const link = search && document.querySelector('a[href="' + article + '"]');
                    return [location.href, document.hasFocus(), document.visibilityState === 'visible',
                            input ? input.value : null,
                            Boolean(link && link.getClientRects().length)];
                }""",
                [step in ("search", "open-article"), self.article],
            )
        except Exception as error:
            raise RouteChanged("Wait for the rehearsal tab to load, then check the route.") from error
        # The browser round trip above delivers queued tab changes to Playwright.
        if any(not tab or tab.is_closed() for tab in tabs):
            raise RouteChanged("A rehearsal tab was closed. Stop and start a fresh run.")
        if self.context.pages != tabs:
            raise RouteChanged("Close extra tabs in the rehearsal window.")
        if current != url and not (step == "open-site" and current in BLANK_TABS):
            name = self.view_names.get(url, f"IANA release {self.parameters.get('release')}")
            raise RouteChanged(f"Return to {name} in the rehearsal tab.")
        if not visible:
            raise RouteChanged("Return to the rehearsal tab in Chrome.")
        if step in ("search", "open-article") and query is None:
            raise RouteChanged("Reload the Wikipedia search page.")
        return focused, query, result

    def verify_route(self, step):
        focused, query, result = self.inspect_route(step)
        if step != "open-site" and self.focus_required and not focused:
            raise RouteChanged("Click inside the rehearsal page in Chrome.")
        if step == "search" and query != "":
            raise RouteChanged("Clear the Wikipedia search field.")
        if step == "open-article":
            if query not in ("", self.query):
                raise RouteChanged("Clear the Wikipedia search field.")
            if query != self.query or not result:
                raise RouteChanged("Select Check route to repeat the Wikipedia search.")

    def restore_route(self, step):
        if step != "open-article":
            self.verify_route(step)
            return
        focused, query, result = self.inspect_route(step)
        if query not in ("", self.query):
            raise RouteChanged("Clear the Wikipedia search field.")
        if query == "" or not result:
            self.search(replace=bool(query))
        elif self.focus_required and not focused:
            self.input.click(self.page, self.page.locator("#searchInput"))
        else:
            return
        self.verify_route(step)

    def result_link(self):
        return self.page.locator(f'a[href="{self.article}"]:visible').first

    def search(self, replace=False):
        from playwright.sync_api import expect

        field = self.page.locator("#searchInput")
        self.input.click(self.page, field)
        if replace:
            field.press("Meta+A" if sys.platform == "darwin" else "Control+A")
            field.press("Backspace")
        field.press_sequentially(self.query, delay=TYPE_DELAY_MS)
        expect(field).to_have_value(self.query)
        self.result_link().wait_for(state="visible")
        self.page.wait_for_timeout(650)

    def open_article(self):
        self.input.click(self.page, self.result_link())
        self.verify_page(self.page, self.article, self.query)

    def follow_article(self, request):
        if not self.link_url:
            raise RuntimeError("Choose the title of a linked article.")
        article_path = self.link_url.removeprefix("https://en.wikipedia.org")
        relative_path = "." + article_path.removeprefix("/wiki")
        selectors = [f'a[href="{path}"]:visible' for path in (article_path, relative_path, self.link_url)]
        link = self.page.locator(", ".join(selectors)).first
        respond(request, "status", waiting=True, detail=f"Waiting for {self.link_title} to open.")
        self.input.click(self.page, link)
        self.verify_page(self.page, self.link_url, self.link_title)

    def close_tabs(self):
        if self.native_chrome:
            for page in self.context.pages:
                with page.expect_event("close", timeout=10000):
                    native_input("close-tab", self.input.chrome_pid, page.title())
        else:
            self.context.close()
            self.context = None

    def section(self, fragment):
        self.page.bring_to_front()
        if self.page.evaluate("location.href.split('#')[0]") != self.article:
            raise RuntimeError("Return to the expected Wikipedia article before selecting its section.")
        self.input.click(self.page, self.page.locator(f'a[href="{fragment}"]:visible').first)
        self.page.locator(fragment).wait_for(state="visible", timeout=10000)
        self.page.wait_for_url(self.article + fragment, timeout=10000)

    def navigation(self, step):
        if step == "article":
            self.section("#Work")
            self.input.wheel(self.page, 800)
        elif step == "data":
            link = self.page.locator("#t-wikibase a")
            if not link.is_visible():
                self.input.click(self.page, self.page.locator("#toc-mw-content-text a"))
                self.input.click(self.page, self.page.locator("#vector-page-tools-dropdown-checkbox"))
            with self.context.expect_page() as opened:
                self.input.click(self.page, link, modifier="Meta" if sys.platform == "darwin" else "Control")
            self.data_tab = opened.value
            self.verify_page(self.data_tab, WIKIDATA, self.query)
            self.data_tab.bring_to_front()
        elif step == "return":
            self.section("#Biography")
            if self.context.pages != [self.page, self.data_tab]:
                raise RuntimeError("The two expected walkthrough tabs are no longer available.")
        else:
            raise RuntimeError(f"Unknown navigation step: {step}")

    def download(self, request):
        from playwright.sync_api import expect

        release = self.parameters["release"]
        step = request["step"]
        if step == "release-history":
            self.input.click(self.page, self.page.get_by_role("link", name="Release History", exact=False))
            self.verify_page(self.page, RELEASES, "Time Zone Database Releases")
        elif step == "release":
            self.input.click(self.page, self.page.get_by_role("link", name=release, exact=True))
            expect(self.page.locator("h1")).to_contain_text(f"Time Zone Database Release {release}")
        elif step == "timer":
            deadline = time.monotonic() + 3
            for seconds in (3, 2, 1):
                respond(request, "status", waiting=True, detail=f"Reading time: {seconds} seconds left.")
                time.sleep(max(0, deadline - time.monotonic() - seconds + 1))
        elif step == "download":
            name = f"tzdata{release}.tar.gz"
            respond(request, "status", waiting=True, detail=f"Waiting for the official {name} download.")
            with self.page.expect_download(timeout=25000) as completed:
                self.input.click(self.page, self.page.get_by_role("link", name=name, exact=True))
            download = completed.value
            if download.suggested_filename != name:
                raise RuntimeError("The browser downloaded a different file.")
            path = Path(request["session_dir"]) / name
            download.save_as(path)
            with tarfile.open(path, "r:gz") as archive:
                version = archive.extractfile("version")
                if version is None or version.read(32).decode().strip() != release:
                    raise RuntimeError("The download does not contain the selected release.")
                if not archive.getmember("europe").isfile():
                    raise RuntimeError("The archive is missing the expected time zone data.")
        elif step == "return":
            self.input.click(self.page, self.page.locator('a[href="/time-zones/releases"]:visible').first)
            self.verify_page(self.page, RELEASES, "Time Zone Database Releases")
        else:
            raise RuntimeError(f"Unknown download step: {step}")

    def check_work(self):
        if self.page.evaluate("location.href") != self.article + "#Work":
            raise RuntimeError("Work is not open. Select Work in the article contents and verify the heading.")
        self.page.get_by_role("heading", name="Work", exact=True).wait_for(state="visible")

    def article_event(self, request):
        step = request["step"]
        if step == "biography":
            self.section("#Biography")
        elif step == "await-work":
            respond(request, "status", waiting=True, detail="Waiting for the article’s Work link.")
            self.page.wait_for_url(self.article + "#Work", wait_until="commit", timeout=30000)
            self.check_work()
        elif step == "check-work":
            self.check_work()
        elif step == "read-work":
            self.check_work()
            self.input.wheel(self.page, 800)
        elif step == "read-article":
            self.verify_page(self.page, self.article, self.query)
            self.input.wheel(self.page, 800)
        else:
            raise RuntimeError(f"Unknown article step: {step}")

    def preview(self, step):
        if not self.context or self.page.is_closed():
            return  # A completed Close leaves the browser closed during cue review.
        self.input.verify_focus()
        if step == "data" and self.data_tab and not self.data_tab.is_closed():
            self.data_tab.bring_to_front()
        elif self.route == "navigation" and step in ("article", "return"):
            self.section("#Work" if step == "article" else "#Biography")
        elif not self.page.is_closed():
            self.page.bring_to_front()

    def run(self, request):
        step = request["step"]
        self.verify_route(step)
        if step == "open-site":
            self.open_site(show_input=True)
        elif step == "search":
            self.search()
        elif step == "open-article":
            self.open_article()
        elif step == "close":
            self.close_tabs()
        elif step == "follow-article":
            self.follow_article(request)
        elif step == "read-section":
            self.section(self.section_fragment)
        elif self.route == "navigation":
            self.navigation(step)
        elif self.route == "download":
            self.download(request)
        else:
            self.article_event(request)

    def cleanup(self):
        try:
            if self.context:
                self.context.close()
        finally:
            if self.playwright:
                self.playwright.stop()


def respond(request, event, **fields):
    print(json.dumps({"id": request["id"], "event": event, **fields}), flush=True)


def main():
    walkthrough = Walkthrough()
    for line in sys.stdin:
        request = json.loads(line)
        operation = request["op"]
        try:
            capabilities = {}
            if operation == "check":
                walkthrough.check()
            elif operation == "prepare" or (operation == "rehearse" and request["step"] is None):
                walkthrough.prepare(request["session_dir"])
                if operation == "prepare":
                    capabilities = {"verify_route": True, "restore_route": True}
            elif operation == "verify":
                walkthrough.verify_route(request["step"])
            elif operation == "restore":
                walkthrough.restore_route(request["step"])
            elif operation == "run":
                walkthrough.run(request)
            elif operation == "preview":
                walkthrough.preview(request["step"])
            elif operation == "rehearse":
                walkthrough.page.bring_to_front()
            elif operation == "cleanup":
                walkthrough.cleanup()
            else:
                raise RuntimeError(f"Unsupported operation: {operation}")
            respond(request, "done", ok=True, **capabilities)
        except RouteChanged as error:
            respond(request, "done", ok=False, route_changed=True, error=str(error))
        except Exception as error:
            respond(request, "done", ok=False, error=str(error).splitlines()[0])
        if operation == "cleanup":
            break


if __name__ == "__main__":
    main()
