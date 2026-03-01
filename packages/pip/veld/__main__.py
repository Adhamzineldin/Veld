"""
Veld CLI — pip wrapper

Uses the bundled binary for the current platform.
Binaries are included in the package for all supported platforms.

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
import shutil
from pathlib import Path


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


def get_bundled_binary_path() -> Path | None:
    """Get the path to the bundled binary for this platform."""
    platform_key = get_platform_key()
    if not platform_key:
        return None
    
    # Binaries are bundled in veld/binaries/{platform}/
    package_dir = Path(__file__).parent
    binary_path = package_dir / "binaries" / platform_key / get_binary_name()
    
    if binary_path.exists():
        return binary_path
    
    return None


def find_binary() -> str:
    """Find the veld binary, using bundled version or PATH."""
    # Try bundled binary first
    bundled = get_bundled_binary_path()
    if bundled:
        return str(bundled)

    # Fallback: check PATH
    which = shutil.which("veld")
    if which:
        return which

    # No binary found
    print("Error: Could not find veld binary.", file=sys.stderr)
    print("The package may be missing binaries for your platform.", file=sys.stderr)
    print("Try reinstalling: pip install --force-reinstall maayn-veld", file=sys.stderr)
    sys.exit(1)


def main():
    """Entry point — find bundled binary, then proxy all args."""
    binary = find_binary()
    args = sys.argv[1:]

    try:
        result = subprocess.run([binary] + args)
        sys.exit(result.returncode)
    except FileNotFoundError:
        print("Error: Could not run veld binary.", file=sys.stderr)
        print("Try reinstalling: pip install --force-reinstall maayn-veld", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        sys.exit(130)


if __name__ == "__main__":
    main()
