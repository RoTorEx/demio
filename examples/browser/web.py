"""Run the public rehearsals with deliberate input in an owned Chrome window."""
import ctypes
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import time


WIKIPEDIA = "https://www.wikipedia.org/"
ARTICLE = "https://en.wikipedia.org/wiki/Ada_Lovelace"
QUERY = "Ada Lovelace"
WIKIDATA = "https://www.wikidata.org/wiki/Q7259"
TIME_ZONES = "https://www.iana.org/time-zones"
RELEASES = "https://www.iana.org/time-zones/releases"
CHROME = Path("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")

class RouteChanged(RuntimeError):
    pass


class Point(ctypes.Structure):
    _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]


def ease(progress):
    return progress * progress * (3 - 2 * progress)


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

    def track_focus(self, session):
        if not self.focus_required:
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
            left, top, frame_width, toolbar, focused = page.evaluate(
                """() => [screenX, screenY, outerWidth - innerWidth,
                          outerHeight - innerHeight, document.hasFocus()]"""
            )
            if not focused and self.focus_required:
                raise RouteChanged("Click inside the rehearsal page in Chrome.")
            if focused and 0 <= frame_width <= 12 and 0 < toolbar < 250:
                origin = (left + frame_width / 2, top + toolbar - 1)
        for step in range(19):
            if step in (6, 12, 18):
                self.verify_focus()
            eased = ease(step / 18)
            px = start_x + (x - start_x) * eased
            py = start_y + (y - start_y) * eased
            if origin:
                self.warp(Point(origin[0] + px, origin[1] + py))
            page.mouse.move(px, py)
            page.wait_for_timeout(20)
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
            page.wait_for_timeout(22)
        page.wait_for_timeout(80)
        if page.evaluate("scrollY") == before:
            raise RuntimeError("The browser view did not scroll.")


class Walkthrough:
    def __init__(self):
        self.parameters = json.loads(os.environ.get("DEMIO_PARAMETERS", "{}"))
        self.route = self.parameters["route"]
        self.headless = os.environ.get("DEMIO_HEADLESS") == "1"
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

    def verify_page(self, page, url, heading):
        from playwright.sync_api import expect

        page.wait_for_url(url, wait_until="domcontentloaded", timeout=25000)
        expect(page.locator("h1")).to_contain_text(heading, timeout=10000)

    def prepare(self, session):
        from playwright.sync_api import sync_playwright

        self.playwright = sync_playwright().start()
        self.context = self.playwright.chromium.launch_persistent_context(
            str(Path(session) / "browser"),
            channel=None if self.headless else "chrome",
            headless=self.headless,
            viewport={"width": 1280, "height": 800},
            locale="en-US",
            color_scheme="light",
        )
        self.context.set_default_timeout(10000)
        self.input.track_focus(session)
        self.page = self.context.pages[0]
        landing = TIME_ZONES if self.route == "download" else WIKIPEDIA
        response = self.page.goto(landing, wait_until="domcontentloaded", timeout=25000)
        if not response or not response.ok:
            raise RuntimeError(f"The public landing page did not load: {landing}")
        if self.route == "download":
            self.verify_page(self.page, TIME_ZONES, "Time Zones")
        else:
            self.page.locator("#searchInput").wait_for(state="visible")
        self.page.bring_to_front()

    def verify_route(self, step):
        self.input.verify_focus()
        url = {
            "search": WIKIPEDIA, "open-article": WIKIPEDIA,
            "article": ARTICLE, "biography": ARTICLE, "read-article": ARTICLE,
            "data": ARTICLE + "#Work", "return": WIKIDATA,
            "await-work": ARTICLE + "#Biography", "check-work": ARTICLE + "#Biography",
            "read-work": ARTICLE + "#Work",
        }.get(step)
        page = self.page
        tabs = [self.page]
        if self.route == "download":
            release = RELEASES + "/" + self.parameters["release"]
            url = {
                "release-history": TIME_ZONES, "release": RELEASES,
                "timer": release, "download": release, "return": release, "close": RELEASES,
            }.get(step)
        elif self.route == "navigation":
            if step == "return":
                page = self.data_tab
            if step in ("return", "close"):
                tabs.append(self.data_tab)
            if step == "close":
                url = ARTICLE + "#Biography"
        elif step == "close":
            url = ARTICLE if self.route == "article" else ARTICLE + "#Work"
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
                [step in ("search", "open-article"), ARTICLE],
            )
        except Exception as error:
            raise RouteChanged("The rehearsal tab is unavailable. Reopen or stop the run.") from error
        if current != url:
            raise RouteChanged(f"Return to {url} in the rehearsal tab.")
        if self.context.pages != tabs:
            raise RouteChanged("Restore the rehearsal tabs in their original order.")
        if not visible:
            raise RouteChanged("Return to the rehearsal tab in Chrome.")
        if self.focus_required and not focused:
            raise RouteChanged("Click inside the rehearsal page in Chrome.")
        if step == "search" and query != "":
            raise RouteChanged("Clear the Wikipedia search field.")
        if step == "open-article" and (query != QUERY or not result):
            raise RouteChanged(f"Enter {QUERY} and leave its search result visible.")

    def result_link(self):
        return self.page.locator(f'a[href="{ARTICLE}"]:visible').first

    def search(self):
        from playwright.sync_api import expect

        field = self.page.locator("#searchInput")
        self.input.click(self.page, field)
        field.press_sequentially(QUERY, delay=75)
        expect(field).to_have_value(QUERY)
        self.result_link().wait_for(state="visible")
        self.page.wait_for_timeout(650)

    def open_article(self):
        self.input.click(self.page, self.result_link())
        self.verify_page(self.page, ARTICLE, QUERY)

    def section(self, fragment):
        self.page.bring_to_front()
        if self.page.evaluate("location.href.split('#')[0]") != ARTICLE:
            raise RuntimeError("Return to the expected Wikipedia article before selecting its section.")
        self.input.click(self.page, self.page.locator(f'a[href="{fragment}"]:visible').first)
        self.page.locator(fragment).wait_for(state="visible", timeout=10000)
        self.page.wait_for_url(ARTICLE + fragment, timeout=10000)

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
            self.verify_page(self.data_tab, WIKIDATA, QUERY)
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
        if self.page.evaluate("location.href") != ARTICLE + "#Work":
            raise RuntimeError("Work is not open. Select Work in the article contents and verify the heading.")
        self.page.get_by_role("heading", name="Work", exact=True).wait_for(state="visible")

    def article_event(self, request):
        step = request["step"]
        if step == "biography":
            self.section("#Biography")
        elif step == "await-work":
            respond(request, "status", waiting=True, detail="Waiting for the article’s Work link.")
            self.page.wait_for_url(ARTICLE + "#Work", wait_until="commit", timeout=30000)
            self.check_work()
        elif step == "check-work":
            self.check_work()
        elif step == "read-work":
            self.check_work()
            self.input.wheel(self.page, 800)
        elif step == "read-article":
            self.verify_page(self.page, ARTICLE, QUERY)
            self.input.wheel(self.page, 800)
        else:
            raise RuntimeError(f"Unknown article step: {step}")

    def preview(self, step):
        if not self.context:
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
        if step == "search":
            self.search()
        elif step == "open-article":
            self.open_article()
        elif step == "close":
            self.context.close()
            self.context = None
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
                    capabilities["verify_route"] = True
            elif operation == "verify":
                walkthrough.verify_route(request["step"])
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
