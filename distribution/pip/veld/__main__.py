"""
Veld CLI - pip wrapper.

On first run this downloads the `veld` binary that matches the package version from the
GitHub release, verifies it against the release's SHA256SUMS, and caches it. A checksum
mismatch is a hard error and nothing is installed.

Release assets (see .github/workflows/release.yml):
    veld-<target-triple>.tar.gz   (veld-x86_64-pc-windows-msvc.zip on Windows)
    SHA256SUMS

Environment:
    VELD_VERSION   override the version to download
    VELD_HOME      cache root (default: ~/.veld)
"""

import hashlib
import io
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import urllib.request
import zipfile
from pathlib import Path
from typing import Optional

from . import __version__

REPO = "Adhamzineldin/Veld"

TARGETS = {
    ("linux", "x86_64"): "x86_64-unknown-linux-musl",
    ("linux", "aarch64"): "aarch64-unknown-linux-musl",
    ("darwin", "x86_64"): "x86_64-apple-darwin",
    ("darwin", "arm64"): "aarch64-apple-darwin",
    ("windows", "amd64"): "x86_64-pc-windows-msvc",
}


def get_target() -> Optional[str]:
    system = platform.system().lower()
    machine = platform.machine().lower()
    if machine == "x64":
        machine = "x86_64"
    if system == "linux" and machine == "arm64":
        machine = "aarch64"
    return TARGETS.get((system, machine))


def binary_name() -> str:
    return "veld.exe" if sys.platform == "win32" else "veld"


def version() -> str:
    return os.environ.get("VELD_VERSION", __version__).lstrip("v")


def cache_dir(target: str) -> Path:
    root = Path(os.environ.get("VELD_HOME") or Path.home() / ".veld")
    return root / "bin" / version() / target


def fetch(url: str) -> bytes:
    with urllib.request.urlopen(url, timeout=60) as response:
        return response.read()


def expected_sha(sums: str, asset: str) -> Optional[str]:
    for line in sums.splitlines():
        parts = line.split()
        if len(parts) == 2 and parts[1].lstrip("*") == asset:
            return parts[0].lower()
    return None


def download(target: str, dest: Path) -> None:
    asset = f"veld-{target}" + (".zip" if sys.platform == "win32" else ".tar.gz")
    base = f"https://github.com/{REPO}/releases/download/v{version()}"
    print(f"veld: downloading {asset} (v{version()})", file=sys.stderr)
    archive = fetch(f"{base}/{asset}")
    sums = fetch(f"{base}/SHA256SUMS").decode("utf-8")
    want = expected_sha(sums, asset)
    if want is None:
        raise RuntimeError(f"{asset} is not listed in SHA256SUMS")
    got = hashlib.sha256(archive).hexdigest()
    if got != want:
        raise RuntimeError(f"checksum mismatch for {asset}: expected {want}, got {got}")

    dest.mkdir(parents=True, exist_ok=True)
    name = binary_name()
    if asset.endswith(".zip"):
        with zipfile.ZipFile(io.BytesIO(archive)) as zf:
            data = zf.read(name)
    else:
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as tf:
            member = next((m for m in tf.getmembers() if os.path.basename(m.name) == name and m.isfile()), None)
            if member is None:
                raise RuntimeError("binary not found in archive")
            extracted = tf.extractfile(member)
            if extracted is None:
                raise RuntimeError("binary not readable in archive")
            data = extracted.read()
    tmp = dest / (name + ".part")
    tmp.write_bytes(data)
    tmp.chmod(0o755)
    tmp.replace(dest / name)


def find_binary() -> str:
    target = get_target()
    if target is None:
        found = shutil.which("veld")
        if found:
            return found
        print(
            "veld: no pre-built binary for this platform. Build from source: "
            f"cargo install --git https://github.com/{REPO} veld",
            file=sys.stderr,
        )
        sys.exit(1)
    dest = cache_dir(target)
    path = dest / binary_name()
    if not path.exists():
        try:
            download(target, dest)
        except Exception as err:  # noqa: BLE001 - report any failure, install nothing
            print(f"veld: install failed: {err}", file=sys.stderr)
            print(f"Download a release manually: https://github.com/{REPO}/releases", file=sys.stderr)
            sys.exit(1)
    return str(path)


def main() -> None:
    binary = find_binary()
    try:
        sys.exit(subprocess.run([binary] + sys.argv[1:]).returncode)
    except KeyboardInterrupt:
        sys.exit(130)


if __name__ == "__main__":
    main()
