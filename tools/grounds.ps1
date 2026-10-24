# The ground lab's capture tour: the same four views of each kind of ground
# (see crates/worldgen/src/grounds.rs), and a contact sheet of them.
#
#   .\tools\grounds.ps1                   every kind
#   .\tools\grounds.ps1 canyons stacked   those
#
# Builds the game first (fast profile). Shots go to screenshots/grounds/<kind>/,
# sheets to screenshots/grounds/<kind>.jpg (needs Python with Pillow).
# Close the game first: a running one locks the build.
param([Parameter(ValueFromRemainingArguments)][string[]]$Kinds)

if (-not $Kinds) { $Kinds = @("tilted", "canyons", "bridged", "nested", "sunken", "contrast", "stacked", "floors", "void_machine", "void_web", "void_lattice", "void_tension", "void_branch", "void_mixed", "void_meander", "void_inlay", "void_inlay_balcony", "void_inlay_screen", "void_inlay_trees") }
Set-Location (Join-Path $PSScriptRoot "..")
if (Get-Process game -ErrorAction SilentlyContinue) { "The game is running: close it first."; exit 1 }
cargo build -p game --profile fast
if ($LASTEXITCODE) { exit 1 }

# The patch's middle is (8192, 8192). Each view: x, height above the
# ground's top, z, yaw, pitch; then any flags. A low sun shows forms best;
# at eye level, the sun high, to see into what is enclosed.
$views = [ordered]@{
    "1 overview" = @("8192,450,9300,0,-24", "--time", "60")
    "2 oblique"  = @("7542,160,8642,305,-14", "--time", "60")
    "3 along"    = @("8192,25,9250,0,-2", "--time", "60")
    "4 eye"      = @("8192,2,8350,0,4")
    "5 eye east" = @("7900,2,8100,270,2")
    "6 eye west" = @("8500,2,8000,90,2")
}
# Per kind, views replaced: canyons' eyes on their floors (4 m), looking
# along them; stacked's and floors' on their floors, under their decks;
# sunken's in its pits.
$canyon = @{
    "4 eye"      = @("8303,6,8351,270,4", "--opt", "camfloor")
    "5 eye east" = @("7787,6,8747,248,4", "--opt", "camfloor")
    "6 eye west" = @("8089,6,7785,68,4", "--opt", "camfloor")
}
# The void kinds stand 1200-1480 m up (worldgen's VOID_LEVELS): their views
# by height over the lab's ground. Over the field; down a corridor between two
# rows of cylinders; beside a collar; down into the void; up from beneath.
$void = @{
    "1 overview" = @("8192,1750,9300,0,-18", "--opt", "camfloor", "--time", "60")
    "2 oblique"  = @("7542,1420,8642,305,-8", "--opt", "camfloor", "--time", "60")
    "3 along"    = @("7600,1305,8267,270,0", "--opt", "camfloor", "--time", "60")
    "4 eye"      = @("8225,1302,8192,270,2", "--opt", "camfloor")
    "5 eye east" = @("8267,1300,8267,270,-60", "--opt", "camfloor")
    "6 eye west" = @("8267,1150,8267,270,40", "--opt", "camfloor")
}
$own = @{
    "canyons" = $canyon
    # (Bridged: one view in a canyon, two on the mesas' tops, among their pits.)
    "bridged" = @{ "4 eye" = $canyon["4 eye"] }
    # (Sunken's on three pits' floors, 82, 99 and 68 m up.)
    "sunken" = @{
        "4 eye"      = @("8787,84,7947,180,6", "--opt", "camfloor")
        "5 eye east" = @("7957,101,8877,270,6", "--opt", "camfloor")
        "6 eye west" = @("8683,70,7571,45,10", "--opt", "camfloor")
    }
    "void_machine" = $void
    "void_web" = $void
    "void_lattice" = $void
    "void_tension" = $void
    "void_branch" = $void
    "void_mixed" = $void
    "void_meander" = $void
    "void_inlay" = $void
    "void_inlay_balcony" = $void
    "void_inlay_screen" = $void
    "void_inlay_trees" = $void
    "floors" = @{
        "4 eye"      = @("8192,6,8350,0,8", "--opt", "camfloor")
        "5 eye east" = @("7900,6,8100,270,8", "--opt", "camfloor")
        "6 eye west" = @("8500,6,8000,90,8", "--opt", "camfloor")
    }
    "stacked" = @{
        "5 eye east" = @("7900,8,8100,270,8", "--opt", "camfloor")
        "6 eye west" = @("8500,8,8000,90,8", "--opt", "camfloor")
    }
}
$sheet = @'
import sys, glob
from PIL import Image, ImageDraw, ImageFont
d, kind = sys.argv[1], sys.argv[2]
try: font = ImageFont.truetype('C:/Windows/Fonts/consolab.ttf', 20)
except OSError: font = ImageFont.load_default()
W, H = 640, 360
out = Image.new('RGB', (W * 3, H * 2))
for k, p in enumerate(sorted(glob.glob(d + '/*.png'))):
    im = Image.open(p).convert('RGB').resize((W, H), Image.LANCZOS)
    name = p.replace('\\', '/').split('/')[-1][2:-4]
    ImageDraw.Draw(im).text((14, 10), kind + ': ' + name, font=font, fill=(255, 255, 255), stroke_width=3, stroke_fill=(0, 0, 0))
    out.paste(im, ((k % 3) * W, (k // 3) * H))
out.save(d + '.jpg', quality=88)
'@
foreach ($kind in $Kinds) {
    $dir = "screenshots/grounds/$kind"
    New-Item -ItemType Directory -Force $dir | Out-Null
    Remove-Item "$dir/*.png" -ErrorAction SilentlyContinue
    foreach ($view in $views.Keys) {
        $v = $views[$view]
        if ($own.Contains($kind) -and $own[$kind].Contains($view)) { $v = $own[$kind][$view] }
        $rest = if ($v.Count -gt 1) { $v[1..($v.Count - 1)] } else { @() }
        & target/fast/game.exe --ground $kind --set mute on --opt clean --cam $v[0] @rest --shot "$dir/$view.png" 2>&1 | Out-Null
    }
    python -I -c $sheet $dir $kind
    "$kind -> $dir.jpg"
}
