"""Run the public browser rehearsals in a fresh Chromium profile."""
import json
import os
from pathlib import Path
import sys
import tarfile
import time


ARTICLE = "https://en.wikipedia.org/wiki/Ada_Lovelace"
RELEASES = "https://www.iana.org/time-zones/releases"


class Walkthrough:
    def __init__(self):
        self.parameters = json.loads(os.environ.get("DEMIO_PARAMETERS", "{}"))
        self.route = self.parameters["route"]
        self.playwright = self.context = self.page = self.data_tab = None

    def check(self):
        from playwright.sync_api import sync_playwright

        with sync_playwright() as probe:
            if not Path(probe.chromium.executable_path).is_file():
                raise RuntimeError("Install pinned Chromium with Playwright before running.")

    def open_page(self, page, url, heading):
        from playwright.sync_api import expect

        response = page.goto(url, wait_until="domcontentloaded", timeout=25000)
        if not response or not response.ok:
            raise RuntimeError(f"The public page did not load: {url}")
        expect(page.locator("h1")).to_contain_text(heading, timeout=10000)

    def prepare(self, session):
        from playwright.sync_api import sync_playwright

        self.playwright = sync_playwright().start()
        self.context = self.playwright.chromium.launch_persistent_context(
            str(Path(session) / "browser"),
            headless=os.environ.get("DEMIO_HEADLESS") == "1",
            viewport={"width": 1280, "height": 800},
            locale="en-US",
            color_scheme="light",
        )
        self.page = self.context.pages[0]
        if self.route == "download":
            self.open_page(self.page, RELEASES, "Time Zone Database Releases")
        else:
            first = self.parameters.get("pages", [{}])[0]
            self.open_page(self.page, first.get("url", ARTICLE), first.get("heading", "Ada Lovelace"))
            self.article_url = self.page.evaluate("location.href.split('#')[0]")

    def section(self, fragment):
        self.page.bring_to_front()
        if self.page.evaluate("location.href.split('#')[0]") != self.article_url:
            raise RuntimeError("Return to the expected Wikipedia article before selecting its section.")
        self.page.locator(f'a[href="{fragment}"]:visible').first.click(timeout=10000)
        self.page.locator(fragment).wait_for(state="visible", timeout=10000)
        if self.page.evaluate("location.href") != self.article_url + fragment:
            raise RuntimeError(f"The article did not open section {fragment}.")

    def navigation(self, step):
        if step == "article":
            self.section("#Work")
        elif step == "data":
            self.data_tab = self.context.new_page()
            target = self.parameters["pages"][1]
            self.open_page(self.data_tab, target["url"], target["heading"])
            self.data_tab.bring_to_front()
        elif step == "return":
            self.section("#Biography")
            if self.context.pages != [self.page, self.data_tab]:
                raise RuntimeError("The two expected tabs are no longer available in order.")
        else:
            raise RuntimeError(f"Unknown navigation step: {step}")

    def download(self, request):
        from playwright.sync_api import expect

        release = self.parameters["release"]
        step = request["step"]
        if step == "release":
            self.page.get_by_role("link", name=release, exact=True).click()
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
                self.page.get_by_role("link", name=name, exact=True).click()
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
            self.page.locator('a[href="/time-zones/releases"]:visible').first.click()
            expect(self.page.locator("h1")).to_have_text("Time Zone Database Releases")
        else:
            raise RuntimeError(f"Unknown download step: {step}")

    def check_work(self):
        # Read the live document after the presenter may have used the browser
        # while the adapter was idle.
        if self.page.evaluate("location.href") != self.article_url + "#Work":
            raise RuntimeError("Work is not open. Select Work in the article contents and verify the heading.")

    def article_event(self, request):
        step = request["step"]
        if step == "biography":
            self.section("#Biography")
        elif step == "await-work":
            respond(request, "status", waiting=True, detail="Waiting for the article’s Work link.")
            self.page.wait_for_function(
                "(url) => location.href === url", arg=self.article_url + "#Work", timeout=30000
            )
        elif step == "check-work":
            self.check_work()
        elif step == "read-work":
            self.check_work()
            self.page.locator("#Work").scroll_into_view_if_needed()
        else:
            raise RuntimeError(f"Unknown article step: {step}")

    def preview(self, step):
        if self.route == "navigation" and step == "data":
            self.data_tab.bring_to_front()
        elif self.route == "navigation":
            self.section("#Work" if step == "article" else "#Biography")
        else:
            self.page.bring_to_front()

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
            if operation == "check":
                walkthrough.check()
            elif operation == "prepare" or (operation == "rehearse" and request["step"] is None):
                walkthrough.prepare(request["session_dir"])
            elif operation == "run":
                if walkthrough.route == "navigation":
                    walkthrough.navigation(request["step"])
                elif walkthrough.route == "download":
                    walkthrough.download(request)
                else:
                    walkthrough.article_event(request)
            elif operation == "preview":
                walkthrough.preview(request["step"])
            elif operation == "rehearse":
                walkthrough.page.bring_to_front()
            elif operation == "cleanup":
                walkthrough.cleanup()
            else:
                raise RuntimeError(f"Unsupported operation: {operation}")
            respond(request, "done", ok=True)
        except Exception as error:
            respond(request, "done", ok=False, error=str(error).splitlines()[0])
        if operation == "cleanup":
            break


if __name__ == "__main__":
    main()
