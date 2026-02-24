param(
  [string]$Profile = "release"
)

$ErrorActionPreference = "Stop"

Write-Host "Building netlab-cli ($Profile)..."
& cargo build -p cli --bin netlab-cli --profile $Profile

Write-Host "Done."
Write-Host "Artifacts:"
$bin = "netlab-cli.exe"
Write-Host "  target/$Profile/$bin"
