$ErrorActionPreference = 'Stop'

$version  = '0.1.0'
$url      = "https://github.com/Adhamzineldin/Veld/releases/download/v${version}/veld-windows-amd64.zip"
$checksum = 'PLACEHOLDER_SHA256_WINDOWS_AMD64'

$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

Install-ChocolateyZipPackage `
  -PackageName   'veld' `
  -Url           $url `
  -UnzipLocation $toolsDir `
  -Checksum      $checksum `
  -ChecksumType  'sha256'
