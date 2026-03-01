"""
Veld CLI — pip wrapper

Downloads the correct pre-built Veld binary for the current platform
from GitHub Releases on first run. Proxies all CLI arguments.

Supported platforms:
  - linux-amd64, linux-arm64
  - darwin-amd64, darwin-arm64
  - windows-amd64
"""

import os
import sys
import platform
import subprocess
import stat
import tarfile
import zipfile
import tempfile
import urllib.request
import shutil
from pathlib import Path

VERSION = "0.1.0"
GITHUB_REPO = "veld-dev/veld"
BASE_URL = f"https://github.com/{GITHUB_REPO}/releases/download/v{VERSION}"


def get_platform_key() -> str | None:
    """Return platform key like 'linux-amd64' or None if unsupported."""
    system = platform.system().lower()
    machine = platform.machine().lower()

    platform_map = {
        "linux": "linux",
        "darwin": "darwin",
        "windows": "windows",
    }

    arch_map = {
        "x86_64": "amd64",
        "amd64": "amd64",
        "arm64": "arm64",
        "aarch64": "arm64",
    }

    p = platform_map.get(system)
    a = arch_map.get(machine)

    if not p or not a:
        return None

    return f"{p}-{a}"


def get_binary_name() -> str:
    """Return binary filename for current platform."""
    return "veld.exe" if sys.platform == "win32" else "veld"


def get_cache_dir() -> Path:
    """Return the platform-appropriate cache directory."""
    if sys.platform == "win32":
        base = Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData" / "Local"))
    elif sys.platform == "darwin":
        base = Path.home() / "Library" / "Caches"
    else:
        base = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache"))
    return base / "veld" / VERSION


def get_binary_path() -> Path:
    """Return the expected path to the cached binary."""
    return get_cache_dir() / get_binary_name()


def download_binary() -> Path:
    """Download and extract the veld binary, returning its path."""
    platform_key = get_platform_key()

    if not platform_key:
        print(
            f"Warning: Unsupported platform {platform.system()}-{platform.machine()}.",
            file=sys.stderr,
        )
        try_go_install()
        sys.exit(1)

    binary_path = get_binary_path()

    # Already downloaded
    if binary_path.exists():
        return binary_path

    ext = ".zip" if sys.platform == "win32" else ".tar.gz"
    url = f"{BASE_URL}/veld-{platform_key}{ext}"

    print(f"Downloading veld {VERSION} for {platform_key}...", file=sys.stderr)
    print(f"  {url}", file=sys.stderr)

    cache_dir = get_cache_dir()
    cache_dir.mkdir(parents=True, exist_ok=True)

    try:
        with tempfile.NamedTemporaryFile(suffix=ext, delete=False) as tmp:
            tmp_path = tmp.name
            urllib.request.urlretrieve(url, tmp_path)

        if ext == ".zip":
            with zipfile.ZipFile(tmp_path, "r") as zf:
                zf.extractall(cache_dir)
        else:
            with tarfile.open(tmp_path, "r:gz") as tf:
                tf.extractall(cache_dir)

        os.unlink(tmp_path)

        # Make executable on Unix
        if sys.platform != "win32":
            binary_path.chmod(binary_path.stat().st_mode | stat.S_IEXEC)

        if binary_path.exists():
            print(f"✓ veld {VERSION} installed", file=sys.stderr)
            return binary_path
        else:
            raise FileNotFoundError("Binary not found after extraction")

    except Exception as e:
        print(f"Warning: Could not download binary: {e}", file=sys.stderr)
        try_go_install()
        sys.exit(1)


def try_go_install():
    """Attempt to install veld via go install."""
    print("Attempting fallback: go install github.com/veld-dev/veld/cmd/veld@latest", file=sys.stderr)
    try:
        subprocess.run(
            ["go", "install", "github.com/veld-dev/veld/cmd/veld@latest"],
            check=True,
        )
        print("✓ Installed veld via go install", file=sys.stderr)
    except (subprocess.CalledProcessError, FileNotFoundError):
        print("", file=sys.stderr)
        print("Install veld manually:", file=sys.stderr)
        print("  go install github.com/veld-dev/veld/cmd/veld@latest", file=sys.stderr)
        print("", file=sys.stderr)
        print("Or download from: https://github.com/veld-dev/veld/releases", file=sys.stderr)


def find_binary() -> str:
    """Find the veld binary, downloading if necessary."""
    # Check cache first
    cached = get_binary_path()
    if cached.exists():
        return str(cached)

    # Check PATH
    which = shutil.which("veld")
    if which:
        return which

    # Download
    return str(download_binary())


def main():
    """Entry point — find or download the binary, then proxy all args."""
    binary = find_binary()
    args = sys.argv[1:]

    try:
        result = subprocess.run([binary] + args)
        sys.exit(result.returncode)
    except FileNotFoundError:
        print("Error: Could not run veld binary.", file=sys.stderr)
        print("Try reinstalling: pip install --force-reinstall veld", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        sys.exit(130)


if __name__ == "__main__":
    main()

