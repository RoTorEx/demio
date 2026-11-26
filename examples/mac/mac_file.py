"""Drive one owned Terminal window and produce a uniquely named sample file."""
import json
import os
from pathlib import Path
import secrets
import shlex
import subprocess
import sys
import time


FIRST = "Demio file walkthrough\n"
SECOND = "Checked in Terminal.\n"
GREETING = "Hello Demio :D\n"
COMMAND_PAUSE_SECONDS = 1


def terminal(script, *arguments):
    result = subprocess.run(
        ["/usr/bin/osascript", "-", *map(str, arguments)],
        input=f'on run argv\ntell application "Terminal"\n{script}\nend tell\nend run\n',
        text=True,
        capture_output=True,
        timeout=15,
    )
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or "Terminal automation failed.")
    return result.stdout.strip()


def identity(path):
    info = path.lstat()
    return info.st_dev, info.st_ino


def native_input(*arguments):
    helper = os.environ.get("DEMIO_NATIVE_INPUT")
    if not helper:
        raise RuntimeError("Run this rehearsal with the Demio native bundle.")
    result = subprocess.run(
        [helper, "--native-input", *arguments], capture_output=True, text=True,
        timeout=15 + 0.15 * sum(map(len, arguments)),
    )
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or "Could not type in the rehearsal Terminal.")


class Walkthrough:
    def __init__(self):
        parameters = json.loads(os.environ.get("DEMIO_PARAMETERS", "{}"))
        self.directory = Path(parameters.get("directory", "~/Desktop")).expanduser().resolve()
        desktop = (Path.home() / "Desktop").resolve()
        self.folder_argument = "~/Desktop" if self.directory == desktop else shlex.quote(str(self.directory))
        self.window = None
        self.tty = None
        self.path = None
        self.owned = None

    def check(self):
        if sys.platform != "darwin":
            raise RuntimeError("The file walkthrough requires macOS Terminal.")
        if not self.directory.is_dir() or not os.access(self.directory, os.W_OK):
            raise RuntimeError(f"Choose an existing writable folder: {self.directory}")
        native_input("check")

    def prepare(self):
        self.check()
        self.path = self.directory / f"demio-{secrets.token_hex(3)}.txt"
        if self.path.exists() or self.path.is_symlink():
            raise RuntimeError("The sample file already exists.")
        window, self.tty = terminal(
            "set previousIDs to id of every window\n"
            'do script ""\n'
            "set demoID to id of front window\n"
            "if demoID is in previousIDs then error \"Terminal did not create a new window.\"\n"
            "activate\n"
            "set demoWindow to window id demoID\n"
            "repeat 50 times\n"
            "set demoTTY to tty of selected tab of demoWindow\n"
            "if demoTTY is not missing value and demoTTY is not \"\" then exit repeat\n"
            "delay 0.1\nend repeat\n"
            "return (demoID as text) & linefeed & (demoTTY as text)",
        ).splitlines()
        self.window = int(window)

    def command(self, text, click=False):
        bounds = terminal(
            "set demoWindow to window id ((item 1 of argv) as integer)\n"
            "if id of front window is not id of demoWindow then error \"Return to the rehearsal Terminal window.\"\n"
            "if tty of selected tab of demoWindow is not (item 2 of argv) then error \"Return to the rehearsal Terminal tab.\"\n"
            "repeat 50 times\n"
            "if not (busy of selected tab of demoWindow) then exit repeat\n"
            "delay 0.1\nend repeat\n"
            "if busy of selected tab of demoWindow then error \"The demo Terminal window is busy.\"\n"
            "return bounds of demoWindow",
            self.window, self.tty,
        )
        native_input("terminal", "click" if click else "type", text, bounds)
        time.sleep(COMMAND_PAUSE_SECONDS)

    def change_directory(self):
        self.command(f"cd {self.folder_argument}", click=True)

    def file_argument(self):
        return f"{self.folder_argument}/{self.path.name}"

    def create_file(self):
        if self.path.exists() or self.path.is_symlink():
            raise RuntimeError("The sample file already exists.")
        self.command(f"touch {self.file_argument()}")
        self.read_expected("")

    def append(self, line, previous=""):
        self.read_expected(previous)
        value = shlex.quote(line.rstrip("\n"))
        self.command(f"echo {value} >> {self.file_argument()}")
        self.read_expected(previous + line)

    def show_file(self, expected):
        self.read_expected(expected)
        self.command(f"cat {self.file_argument()}")
        if expected:
            contents = terminal(
                "set demoWindow to window id ((item 1 of argv) as integer)\n"
                "if tty of selected tab of demoWindow is not (item 2 of argv) then error \"Return to the rehearsal Terminal tab.\"\n"
                "return contents of selected tab of demoWindow",
                self.window, self.tty,
            )
            lines = "\n".join(contents.splitlines())
            message = expected.rstrip("\n")
            if f"\n{message}\n" not in f"\n{lines}\n":
                raise RuntimeError("The greeting did not appear in Terminal.")

    def read_expected(self, text):
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if self.path.is_file() and not self.path.is_symlink():
                current = identity(self.path)
                if self.owned is not None and current != self.owned:
                    raise RuntimeError("The sample file was replaced.")
                if self.path.read_text() == text:
                    self.owned = current
                    return
            time.sleep(0.1)
        raise RuntimeError("Terminal did not produce the expected sample file contents.")

    def run(self, step):
        if step == "cd":
            self.change_directory()
        elif step == "touch":
            self.create_file()
        elif step == "empty":
            self.show_file("")
        elif step == "write":
            self.append(GREETING)
        elif step == "read":
            self.show_file(GREETING)
        elif step == "create":
            self.change_directory()
            self.create_file()
            self.append(FIRST)
            self.show_file(FIRST)
        elif step == "append":
            self.append(SECOND, FIRST)
            self.show_file(FIRST + SECOND)
        elif step == "open":
            self.read_expected(FIRST + SECOND)
            self.open_document()
        else:
            raise RuntimeError(f"Unknown Mac step: {step}")

    def open_document(self):
        subprocess.run(["/usr/bin/open", "-a", "TextEdit", str(self.path)], check=True, timeout=10)

    def preview(self, step):
        if step == "open":
            self.read_expected(FIRST + SECOND)
            self.open_document()
        else:
            terminal("set index of window id ((item 1 of argv) as integer) to 1\nactivate", self.window)

    def cleanup(self):
        if self.window is not None:
            terminal(
                "set demoWindow to window id ((item 1 of argv) as integer)\n"
                "if (count tabs of demoWindow) is 1 and (tty of selected tab of demoWindow) is (item 2 of argv) then\n"
                "if not (busy of selected tab of demoWindow) then close demoWindow\nend if",
                self.window, self.tty,
            )


def main():
    walkthrough = Walkthrough()
    for line in sys.stdin:
        request = json.loads(line)
        operation = request["op"]
        reply = {"id": request["id"], "event": "done", "ok": True}
        try:
            if operation == "check":
                walkthrough.check()
            elif operation == "prepare":
                walkthrough.prepare()
            elif operation == "run":
                walkthrough.run(request["step"])
            elif operation == "preview":
                walkthrough.preview(request["step"])
            elif operation == "cleanup":
                walkthrough.cleanup()
            else:
                raise RuntimeError(f"Unsupported operation: {operation}")
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            reply.update(ok=False, error=str(error))
        print(json.dumps(reply), flush=True)
        if operation == "cleanup":
            break


if __name__ == "__main__":
    main()
