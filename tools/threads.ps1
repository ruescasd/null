# CPU per named thread of a running process, sampled every $Every seconds:
# each interval, the cores' worth used by each group of threads (by name,
# digits stripped). Usage: threads.ps1 -Id PID -Seconds N -Every S
param([int]$Id, [int]$Seconds = 60, [double]$Every = 2)
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class T {
  [DllImport("kernel32.dll")] public static extern IntPtr OpenThread(uint access, bool inherit, uint id);
  [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
  [DllImport("kernel32.dll")] public static extern int GetThreadDescription(IntPtr h, out IntPtr desc);
  [DllImport("kernel32.dll")] public static extern IntPtr LocalFree(IntPtr p);
  [DllImport("kernel32.dll")] public static extern bool GetThreadTimes(IntPtr h, out long c, out long e, out long k, out long u);
  public static string Name(uint id) {
    IntPtr h = OpenThread(0x0800, false, id); if (h == IntPtr.Zero) return "?";
    IntPtr d; string s = "?"; if (GetThreadDescription(h, out d) >= 0 && d != IntPtr.Zero) { s = Marshal.PtrToStringUni(d); LocalFree(d); }
    CloseHandle(h); return s;
  }
  public static long Cpu(uint id) {
    IntPtr h = OpenThread(0x0800, false, id); if (h == IntPtr.Zero) return 0;
    long c, e, k, u; GetThreadTimes(h, out c, out e, out k, out u); CloseHandle(h); return k + u;
  }
}
"@
function Sample {
  $p = Get-Process -Id $Id -ErrorAction SilentlyContinue
  if (-not $p) { return $null }
  $m = @{}
  foreach ($t in $p.Threads) { $n = [T]::Name([uint32]$t.Id); if (-not $n) { $n = "(unnamed)" }; $g = ($n -replace '\d+', '#'); $m[$g] = $m[$g] + [T]::Cpu([uint32]$t.Id) }
  $m
}
$prev = Sample; $t0 = Get-Date; $prevT = $t0
while (((Get-Date) - $t0).TotalSeconds -lt $Seconds) {
  Start-Sleep -Milliseconds ([int]($Every * 1000))
  $now = Sample; if (-not $now) { break }
  $nowT = Get-Date; $dt = ($nowT - $prevT).TotalSeconds
  $parts = foreach ($k in ($now.Keys | Sort-Object)) { $c = ($now[$k] - $prev[$k]) / 1e7 / $dt; if ($c -ge 0.05) { "{0} {1:N2}" -f $k, $c } }
  $total = (($now.Values | Measure-Object -Sum).Sum - ($prev.Values | Measure-Object -Sum).Sum) / 1e7 / $dt
  "{0,5:N0}s total {1:N2} cores | {2}" -f ($nowT - $t0).TotalSeconds, $total, ($parts -join " | ")
  $prev = $now; $prevT = $nowT
}
