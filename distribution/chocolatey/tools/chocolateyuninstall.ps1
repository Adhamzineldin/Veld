$ErrorActionPreference = 'Stop'

$toolsDir = "$(Split-Path -Parent $MyInvocation.MyCommand.Definition)"
Remove-Item "$toolsDir\veld.exe" -ErrorAction SilentlyContinue
