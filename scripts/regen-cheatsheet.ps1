# Regenerate the committed cheatsheet from px-ast / grammar.pest / the real
# .px corpus into docs/px-grammar-cheatsheet.md (C-DRIFT-001, mirrors
# regen-schema.ps1). Never hand-edit the generated file.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $outFile = Join-Path (Join-Path $root "docs") "px-grammar-cheatsheet.md"
    & cargo run -q -p px-cheatsheet -- "$outFile"
    if ($LASTEXITCODE -ne 0) { throw "px-cheatsheet-gen failed (exit $LASTEXITCODE)" }
    Write-Host ("regenerated {0} ({1} bytes)" -f $outFile, (Get-Item $outFile).Length)
}
finally { Pop-Location }
