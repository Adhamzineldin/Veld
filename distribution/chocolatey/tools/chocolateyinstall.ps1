$ErrorActionPreference = 'Stop'

# `package.sh choco` replaces the version and checksum from the release's SHA256SUMS.
# The Chocolatey package version is the release version without dots in the prerelease
# (2.0.0-beta.1 is 2.0.0-beta1), so the download version is kept separately here.
$releaseVersion = '2.0.0-beta.1'
$url            = "https://github.com/Adhamzineldin/Veld/releases/download/v$releaseVersion/veld-x86_64-pc-windows-msvc.zip"
$checksum       = 'PLACEHOLDER_SHA256_X86_64_PC_WINDOWS_MSVC'

$toolsDir = "$(Split-Path -Parent $MyInvocation.MyCommand.Definition)"

# Chocolatey verifies the SHA256 of the download and fails the install on a mismatch.
Install-ChocolateyZipPackage `
  -PackageName   'veld' `
  -Url64bit      $url `
  -UnzipLocation $toolsDir `
  -Checksum64    $checksum `
  -ChecksumType64 'sha256'
