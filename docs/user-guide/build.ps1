# Builds the user guide PDF from guide.html using Microsoft Edge (headless) and writes it to
# assets\user-guide.pdf, which the program embeds. Run from anywhere:
#   powershell -File docs\user-guide\build.ps1
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = Resolve-Path (Join-Path $here '..\..')
$html = Join-Path $here 'guide.html'
$out  = Join-Path $root 'assets\user-guide.pdf'
$edge = @("${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe", "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { throw 'Microsoft Edge was not found.' }
$profile = Join-Path $env:TEMP 'tideline-guide-edge'
$url = ([System.Uri]$html).AbsoluteUri
if (Test-Path $out) { Remove-Item $out }
$edgeArgs = @('--headless=new', '--disable-gpu', "--user-data-dir=$profile", '--no-pdf-header-footer', '--generate-pdf-document-outline', "--print-to-pdf=`"$out`"", $url)
Start-Process -FilePath $edge -ArgumentList $edgeArgs -Wait -NoNewWindow -RedirectStandardError (Join-Path $env:TEMP 'tideline-guide-edge.log')
# Edge hands the work to another process, so wait for the file to appear and stop growing.
$waited = 0
while (-not (Test-Path $out) -and $waited -lt 90) { Start-Sleep -Milliseconds 500; $waited++ }
if (-not (Test-Path $out)) { throw 'The PDF was not created.' }
$last = -1
while ((Get-Item $out).Length -ne $last) { $last = (Get-Item $out).Length; Start-Sleep -Seconds 1 }
"Created $out ({0:N1} MB)" -f ((Get-Item $out).Length / 1MB)
