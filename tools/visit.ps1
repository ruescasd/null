# Opens the game at one of the places shown on the review pages, with the
# same camera as the capture, flying (V to land and walk).
#
#   .\tools\visit.ps1 <place>        (no place: lists them)
#   .\tools\visit.ps1 lab <name>     straight to a lab candidate, e.g. organ_dressed
#                                    (names: cargo run -p worldgen --release --example lab)
#
# In the game: Shift flies fast, Left/Right scrub the time of day, T pauses
# the suns, F12 saves a screenshot to screenshots/.
param([string]$Place, [string]$Name)

# name = x, height above the ground, z, yaw, pitch (degrees)
$places = [ordered]@{
    "horizon"     = "1200,30,900,-87.6,6.1"
    "needles"     = "2381,40,1094,-58.2,22.0"
    "mountain"    = "4526,50,1562,-63.4,6.0"
    "ruin"        = "4372,60,3397,-59.0,13.5"
    "enclosure"   = "859,320,3333,-45.0,-26.7"
    "town"        = "14196,120,2921,-45.0,-26.5"
    "town_street" = "14250,35,2600,-142.5,-6.0"
    "plaza"       = "1822,70,639,-45.0,-18.3"
    "towers"      = "2450,30,560,-59.3,-4.0"
    "pillars"     = "2350,25,1150,108.4,-2.7"
    "spire"       = "2333,30,2204,-56.3,17.0"
    # The lab: candidates in a row along +x on flat ground (--opt lab).
    "lab"         = "1350,40,700,-68.2,-4.2"
}

if (-not $Place -or -not $places.Contains($Place)) {
    "Places: " + ($places.Keys -join ", ")
    exit 1
}
Set-Location (Join-Path $PSScriptRoot "..")
$extra = if ($Place -eq "lab") { @("--opt", "lab") } else { @() }
if ($Place -eq "lab" -and $Name) { $extra += @("--focus", $Name) }
cargo run -p game --release -- --opt noclip --time 40 --cam $places[$Place] @extra
