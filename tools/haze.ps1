# Runs the game with one of the haze variants from the second review, all
# from the same spot: on foot near the shattered-shaft colossus (Broken),
# with the mountain colossus and the stacks beyond it.
#
#   .\tools\haze.ps1 none | light | dense | dark
#
# In the game: V toggles flying (Shift fast), Left/Right scrub the time of
# day (the haze follows the daylight), T pauses the suns, F12 saves a
# screenshot to screenshots/.
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("none", "light", "dense", "dark")]
    [string]$Variant
)

$haze = @{
    "none"  = @("--set", "fog=0")
    "light" = @("--set", "fog=7000", "--set", "fog_day=0.25")
    "dense" = @("--set", "fog=3500", "--set", "fog_day=0.3")
    "dark"  = @("--set", "fog=4500", "--set", "fog_day=0.04", "--set", "fog_glow=0.15")
}[$Variant]

Set-Location (Join-Path $PSScriptRoot "..")
cargo run -p game --release -- --cam "4372,2,3397,-59,10" --time 40 @haze
