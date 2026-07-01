# Regenerate the committed schema artifacts from px-ast into schema/.
# ADR §M4 / C-DRIFT-001: schema/px.schema.json + schema/px.schema.px are
# GENERATED projections of px-ast (via schemars). Never hand-edit them.
#
# The generator writes the files DIRECTLY (we pass the out-dir) instead of
# capturing stdout in PowerShell, because PowerShell decodes child-process
# stdout with the active console code page and corrupts multibyte UTF-8
# characters (em dash, box-drawing). Direct file writes emit raw UTF-8/LF.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $outDir = Join-Path $root "schema"
    & cargo run -q -p px-schema -- "$outDir"
    if ($LASTEXITCODE -ne 0) { throw "px-schema-gen failed (exit $LASTEXITCODE)" }
    $json = Join-Path $outDir "px.schema.json"
    $px = Join-Path $outDir "px.schema.px"
    Write-Host ("regenerated {0} ({1} bytes)" -f $json, (Get-Item $json).Length)
    Write-Host ("regenerated {0} ({1} bytes)" -f $px, (Get-Item $px).Length)
}
finally { Pop-Location }
