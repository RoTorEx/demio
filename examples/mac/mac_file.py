"""Drive one owned Terminal window and produce a uniquely named sample file."""
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import time


FIRST = "Demio file walkthrough\n"
SECOND = "Checked in Terminal.\n"


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


class Walkthrough:
    def __init__(self):
        parameters = json.loads(os.environ.get("DEMIO_PARAMETERS", "{}"))
        self.directory = Path(parameters.get("directory", "~/Desktop")).expanduser().resolve()
        self.window = None
        self.path = None
        self.owned = None
        self.opened = False

    def check(self):
        if sys.platform != "darwin":
            raise RuntimeError("The file walkthrough requires macOS Terminal and TextEdit.")
        if not self.directory.is_dir() or not os.access(self.directory, os.W_OK):
            raise RuntimeError(f"Choose an existing writable folder: {self.directory}")

    def prepare(self, session):
        self.check()
        self.path = self.directory / f"Demio example {Path(session).name}.txt"
        if self.path.exists() or self.path.is_symlink():
            raise RuntimeError("The sample file already exists.")
        ready = Path(session) / "terminal-ready"
        command = shlex.join([
            "/bin/sh", "-c", 'cd "$1" && /bin/pwd && /usr/bin/touch "$2"',
            "demio", str(self.directory), str(ready),
        ])
        self.window = int(terminal(
            "set previousIDs to id of every window\n"
            "do script (item 1 of argv)\n"
            "set demoID to id of front window\n"
            "if demoID is in previousIDs then error \"Terminal did not create a new window.\"\n"
            "activate\nreturn demoID",
            command,
        ))
        deadline = time.monotonic() + 10
        while not ready.is_file():
            if time.monotonic() >= deadline:
                raise RuntimeError("The demo Terminal window did not reach its first prompt.")
            time.sleep(0.1)

    def command(self, command, *arguments):
        shell = shlex.join(["/bin/sh", "-c", command, "demio", *map(str, arguments)])
        terminal(
            "set demoWindow to window id ((item 1 of argv) as integer)\n"
            "repeat 50 times\n"
            "if not (busy of selected tab of demoWindow) then exit repeat\n"
            "delay 0.1\nend repeat\n"
            "if busy of selected tab of demoWindow then error \"The demo Terminal window is busy.\"\n"
            "set index of demoWindow to 1\nactivate\n"
            "do script (item 2 of argv) in selected tab of demoWindow",
            self.window, shell,
        )

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
        if step == "create":
            self.command('set -C\n/usr/bin/printf "%s" "$2" > "$1"\n/bin/cat "$1"', self.path, FIRST)
            self.read_expected(FIRST)
        elif step == "append":
            self.read_expected(FIRST)
            self.command('/usr/bin/printf "%s" "$2" >> "$1"\n/bin/cat "$1"', self.path, SECOND)
            self.read_expected(FIRST + SECOND)
        elif step == "open":
            self.read_expected(FIRST + SECOND)
            self.open_document()
            self.opened = True
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
        try:
            if self.window is not None:
                terminal(
                    "set demoWindow to window id ((item 1 of argv) as integer)\n"
                    "if not (busy of selected tab of demoWindow) then close demoWindow",
                    self.window,
                )
        finally:
            if self.owned is not None and not self.opened and self.path.is_file():
                if identity(self.path) == self.owned and self.path.read_text() in (FIRST, FIRST + SECOND):
                    self.path.unlink()


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
                walkthrough.prepare(request["session_dir"])
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
