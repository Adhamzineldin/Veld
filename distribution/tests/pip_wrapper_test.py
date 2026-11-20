"""Test of the pip wrapper (distribution/pip) against real release artifacts.

    python distribution/tests/pip_wrapper_test.py <artifacts-dir>

<artifacts-dir> holds the release assets (veld-<target>.tar.gz|zip and SHA256SUMS, for example the
artifacts of a release.yml dry run). They are served from a local HTTP server and the wrapper is
pointed at it with VELD_DOWNLOAD_BASE (cache in a temp VELD_HOME). Checks, for this platform:
the right asset is downloaded and verified and `python -m veld --version` runs it; a tampered
archive is rejected and nothing is cached; an asset missing from SHA256SUMS is rejected.
"""

import functools
import http.server
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path

PIP_DIR = Path(__file__).resolve().parent.parent / "pip"
ARTIFACTS = Path(sys.argv.pop(1)).resolve() if len(sys.argv) > 1 else None


class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):  # noqa: D102
        pass


def serve(directory: Path):
    handler = functools.partial(Quiet, directory=str(directory))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, f"http://127.0.0.1:{server.server_address[1]}"


class PipWrapper(unittest.TestCase):
    def setUp(self):
        if ARTIFACTS is None or not (ARTIFACTS / "SHA256SUMS").is_file():
            self.fail("usage: pip_wrapper_test.py <artifacts-dir containing SHA256SUMS>")
        self.tmp = Path(tempfile.mkdtemp(prefix="veld-pip-test-"))
        self.addCleanup(shutil.rmtree, self.tmp, True)
        sys.path.insert(0, str(PIP_DIR))
        self.addCleanup(sys.path.remove, str(PIP_DIR))
        from veld import __main__ as wrapper

        self.w = wrapper
        self.target = wrapper.get_target()
        if self.target is None:
            self.skipTest("no release asset for this platform")
        self.asset = f"veld-{self.target}" + (".zip" if sys.platform == "win32" else ".tar.gz")

    def mirror(self, name, edit):
        d = self.tmp / name
        shutil.copytree(ARTIFACTS, d)
        edit(d)
        return d

    def run_wrapper(self, base, home, *args):
        env = dict(os.environ, VELD_DOWNLOAD_BASE=base, VELD_HOME=str(home), PYTHONPATH=str(PIP_DIR))
        return subprocess.run(
            [sys.executable, "-I", "-c", "import sys; sys.path.insert(0, %r); from veld.__main__ import main; main()" % str(PIP_DIR), *args],
            env=env, capture_output=True, text=True, timeout=180, stdin=subprocess.DEVNULL,
        )

    def test_installs_verifies_and_runs(self):
        server, base = serve(ARTIFACTS)
        self.addCleanup(server.shutdown)
        r = self.run_wrapper(base, self.tmp / "home", "--version")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn(self.asset, r.stderr)
        self.assertIn("2.0.0", r.stdout)

    def test_tampered_archive_is_rejected(self):
        d = self.mirror("tampered", lambda d: (d / self.asset).open("ab").write(b"\0"))
        server, base = serve(d)
        self.addCleanup(server.shutdown)
        home = self.tmp / "home"
        r = self.run_wrapper(base, home, "--version")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("checksum mismatch", r.stderr)
        self.assertFalse(list(home.rglob("veld*")) if home.exists() else [], "something was cached")

    def test_asset_missing_from_sums_is_rejected(self):
        def drop(d):
            lines = (d / "SHA256SUMS").read_text().splitlines()
            (d / "SHA256SUMS").write_text("\n".join(l for l in lines if self.asset not in l) + "\n")

        server, base = serve(self.mirror("unlisted", drop))
        self.addCleanup(server.shutdown)
        r = self.run_wrapper(base, self.tmp / "home", "--version")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("not listed in SHA256SUMS", r.stderr)


if __name__ == "__main__":
    unittest.main()
