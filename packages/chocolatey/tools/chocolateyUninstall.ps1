$ErrorActionPreference = 'Stop'

$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"
Remove-Item "$toolsDir\veld.exe" -ErrorAction SilentlyContinue
