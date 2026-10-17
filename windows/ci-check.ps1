# Checks of the Windows build on a GitHub Actions runner (windows-latest), with no input.
# Every check throws on failure; screenshots and logs land in .\windows-check for review.
#   pwsh windows/ci-check.ps1 -Version 0.4.0 -Setup dist-installer\Ryuon-0.4.0-Setup.exe
param(
    [Parameter(Mandatory)] [string] $Version,
    [Parameter(Mandatory)] [string] $Setup
)
$ErrorActionPreference = 'Stop'
$Out = Join-Path $PWD 'windows-check'
New-Item -ItemType Directory -Force $Out | Out-Null
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$Clean = "C:\Windows\system32;C:\Windows"   # PATH of a PC without any developer tools
$Results = [System.Collections.Generic.List[string]]::new()

function Check([string] $Name, [bool] $Ok, [string] $Detail = '') {
    if ($Ok) { $Results.Add("PASS $Name"); Write-Host "PASS $Name" }
    else { Write-Host "FAIL $Name $Detail"; $Results | Out-File "$Out\results.txt"; throw "FAIL $Name $Detail" }
}

function Shot([string] $Name) {
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    [System.Drawing.Graphics]::FromImage($bmp).CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
    $bmp.Save("$Out\$Name.png")
}

# Starts ryuon.exe with a clean PATH and its own settings folder, waits, screenshots, stops it
function Run-Ryuon([string] $Exe, [string] $Name, [string[]] $Arguments = @(), [int] $Seconds = 15) {
    $env:PATH = $Clean
    $p = if ($Arguments.Count) {
        Start-Process -FilePath $Exe -ArgumentList $Arguments -PassThru -RedirectStandardError "$Out\$Name.log"
    } else {
        Start-Process -FilePath $Exe -PassThru -RedirectStandardError "$Out\$Name.log"
    }
    Start-Sleep -Seconds $Seconds
    Shot $Name
    $alive = -not $p.HasExited
    if ($alive) { Stop-Process -Id $p.Id -Force; Start-Sleep 2 }
    Check "$Name`: Ryuon is running after $Seconds s" $alive "exit code $($p.ExitCode)"
}

# Settings in a keyfile under a throwaway folder instead of the registry
function Use-Settings([hashtable] $Values) {
    $env:GSETTINGS_BACKEND = 'keyfile'
    $env:XDG_CONFIG_HOME = Join-Path $PWD 'test-config'
    $dir = Join-Path $env:XDG_CONFIG_HOME 'glib-2.0\settings'
    Remove-Item -Recurse -Force $env:XDG_CONFIG_HOME -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $dir | Out-Null
    $lines = @('[io/github/k_s_harish_balagi/Ryuon]') + ($Values.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" })
    $lines | Out-File -Encoding utf8NoBOM "$dir\keyfile"
}

# --- 1. The installer itself
$info = (Get-Item $Setup).VersionInfo
Check 'installer exists' (Test-Path $Setup)
Check "installer is version $Version" ($info.ProductVersion -like "$Version*") $info.ProductVersion
Write-Host ("installer size: {0:N1} MB" -f ((Get-Item $Setup).Length / 1MB))

# --- 2. Silent install for the current user (no administrator rights)
$app = Join-Path $env:LOCALAPPDATA 'Programs\Ryuon'
$p = Start-Process $Setup -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CURRENTUSER', "/LOG=$Out\install.log" -PassThru -Wait
Check 'install exits 0' ($p.ExitCode -eq 0) "exit $($p.ExitCode)"
$exe = Join-Path $app 'bin\ryuon.exe'
Check 'installed into the user programs folder' (Test-Path $exe) $exe
$lnk = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Ryuon.lnk'
Check 'Start menu entry' (Test-Path $lnk)
$target = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk).TargetPath
Check 'Start menu entry opens ryuon.exe' ($target -eq $exe) $target
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{E0971405-A384-4B85-9D36-8F3D0B8279D4}_is1'
Check 'listed in Installed apps' (Test-Path $key)
Check "Installed apps shows version $Version" ((Get-ItemProperty $key).DisplayVersion -eq $Version) (Get-ItemProperty $key).DisplayVersion

# --- 3. The exe: version details and icon
$v = (Get-Item $exe).VersionInfo
Check 'ryuon.exe product name' ($v.ProductName -eq 'Ryuon') $v.ProductName
Check "ryuon.exe version $Version" ($v.ProductVersion -eq $Version) $v.ProductVersion
$icon = [System.Drawing.Icon]::ExtractAssociatedIcon($exe)
$icon.ToBitmap().Save("$Out\exe-icon.png")
Check 'ryuon.exe has its own icon' ($icon.Width -ge 32)

# --- 4. Opens, from the Start menu target, with nothing else on PATH
Use-Settings @{}
Run-Ryuon $exe 'installed-first-start' -Seconds 20

# --- 5. A music library: folder by folder, tidy names, resume point
$lib = Join-Path $PWD 'TestLibrary\Music'
Remove-Item -Recurse -Force (Split-Path $lib) -ErrorAction SilentlyContinue
$media = Join-Path $PWD 'windows\test-media'
$layout = @{
    'Album One - MassTamilan.dev\01 First-MassTamilan.dev.mp3' = 'tone.mp3'
    'Album One - MassTamilan.dev\02 Second-MassTamilan.dev.flac' = 'tone.flac'
    'Album Two 320kbps\01 Third.ogg' = 'tone.ogg'
    'Album Two 320kbps\02 Fourth.opus' = 'tone.opus'
    'Album Three (2024)\01 Fifth.m4a' = 'tone.m4a'
}
foreach ($rel in $layout.Keys) {
    $dest = Join-Path $lib $rel
    New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
    Copy-Item (Join-Path $media $layout[$rel]) $dest
}
Use-Settings @{ 'library-path' = "'" + $lib.Replace('\', '\\') + "'"; 'library-autoload' = 'true' }
Run-Ryuon $exe 'library-folder-by-folder' -Seconds 20
# Ryuon keeps its cache in %LOCALAPPDATA%\Ryuon\cache (GLib's default, INetCache, is emptied by Disk Cleanup)
$playlist = Join-Path $env:LOCALAPPDATA 'Ryuon\cache\ryuon\playlists\current.pls'
Check 'library queued (saved playlist written)' (Test-Path $playlist) $playlist
$entries = (Select-String -Path $playlist -Pattern '^File\d+=').Count
Check 'all 5 songs from 3 folders queued' ($entries -eq 5) "$entries entries"
$files = Select-String -Path $playlist -Pattern '^File\d+=(.*)$' | ForEach-Object { $_.Matches[0].Groups[1].Value }
Check 'queue is in folder order' ($files[0] -like '*Album One*01 First*' -and $files[4] -like '*Album Two*02 Fourth*') ($files -join ' | ')

# --- 6. Upgrade over a running copy: the installer closes it and replaces the files
$env:PATH = $Clean
$running = Start-Process -FilePath $exe -PassThru
Start-Sleep 8
$p = Start-Process $Setup -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CURRENTUSER', '/CLOSEAPPLICATIONS', "/LOG=$Out\upgrade.log" -PassThru -Wait
Check 'upgrade over a running copy exits 0' ($p.ExitCode -eq 0) "exit $($p.ExitCode)"
Check 'running copy was closed for the upgrade' ($running.HasExited)
if (-not $running.HasExited) { Stop-Process -Id $running.Id -Force }
Check 'still installed after the upgrade' (Test-Path $exe)
Run-Ryuon $exe 'after-upgrade' -Seconds 15

# --- 7. Uninstall removes the program and its entries, keeps the user's music
$p = Start-Process (Join-Path $app 'unins000.exe') -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -PassThru -Wait
# The uninstaller hands over to a copy of itself in TEMP and returns at once: wait for the files
for ($i = 0; $i -lt 60 -and (Test-Path $exe); $i++) { Start-Sleep 1 }
Check 'uninstall exits 0' ($p.ExitCode -eq 0) "exit $($p.ExitCode)"
Check 'program files removed' (-not (Test-Path $exe))
Check 'Start menu entry removed' (-not (Test-Path $lnk))
Check 'removed from Installed apps' (-not (Test-Path $key))
Check "the user's music is untouched" ((Get-ChildItem $lib -Recurse -File).Count -eq 5)

$Results | Out-File "$Out\results.txt"
Write-Host "`n$($Results.Count) checks passed"
