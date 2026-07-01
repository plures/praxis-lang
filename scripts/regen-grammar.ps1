# Regenerate the canonical grammar.pest from px-grammar-gen into px-grammar/src/.
# ADR-0021: grammar.pest is a GENERATED, committed artifact. Never hand-edit.
#
# We let the generator write the file DIRECTLY (passing the target path) instead
# of capturing stdout in PowerShell, because PowerShell decodes child-process
# stdout with the active console code page and corrupts the grammar's multibyte
# UTF-8 characters (em dash, box-drawing). Direct file write emits raw UTF-8/LF.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $target = Join-Path $root "crates/px-grammar/src/grammar.pest"
    & cargo run -q -p px-grammar-gen -- "$target"
    if ($LASTEXITCODE -ne 0) { throw "px-grammar-gen failed (exit $LASTEXITCODE)" }
    $bytes = (Get-Item $target).Length
    Write-Host "regenerated $target ($bytes bytes)"
}
finally { Pop-Location }
