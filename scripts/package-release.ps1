# ListenForge 发布打包脚本 / Release packaging
# 用法: pwsh scripts/package-release.ps1 [-Version 0.1.2]
# 前提: 已完成双架构构建
#   npm run tauri build                                      # ARM64 (native)
#   npm run tauri build -- --target x86_64-pc-windows-msvc   # x64
# 产物输出到 dist-release/ : 2 安装版 exe + 2 绿色版 zip

param([string]$Version)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

if (-not $Version) {
  $conf = Get-Content "src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json
  $Version = $conf.version
}
Write-Host "打包版本 / Version: $Version`n"

$out = Join-Path $root "dist-release"
Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $out | Out-Null

$readme = @"
ListenForge 绿色版 / Portable Edition
=====================================
1. 把本文件夹解压到任意位置(如桌面),无需安装。
   Unzip anywhere (e.g. Desktop). No installation needed.
2. 双击 listenforge.exe 运行。
   Double-click listenforge.exe to run.
3. 首次使用配置 AI:程序内右上「AI 设置」填 API 地址 / 密钥 / 模型,
   或编辑 文档\ListenForge\.env。
   First run: set the API URL / key / model in the in-app "AI Settings".
4. pdfium.dll 必须和 listenforge.exe 放在同一文件夹(用于读取 PDF)。
   Keep pdfium.dll next to listenforge.exe (needed to read PDFs).

主页 / Home: https://github.com/s-silt/ListenForge
"@

function Pack-Portable($arch, $exePath, $dll) {
  if (-not (Test-Path $exePath)) { throw "缺少 exe: $exePath" }
  if (-not (Test-Path $dll))     { throw "缺少 dll: $dll" }
  $stage = Join-Path $out "stage_$arch"
  New-Item -ItemType Directory -Force $stage | Out-Null
  Copy-Item $exePath (Join-Path $stage "listenforge.exe")
  Copy-Item $dll     (Join-Path $stage "pdfium.dll")
  $readme | Set-Content (Join-Path $stage "使用说明_README.txt") -Encoding utf8
  $zip = Join-Path $out "ListenForge_${Version}_${arch}_portable.zip"
  Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -Force
  Remove-Item $stage -Recurse -Force
  Write-Host "  绿色版 / portable: $(Split-Path $zip -Leaf)"
}

function Collect-Setup($setupPath) {
  if (-not (Test-Path $setupPath)) { throw "缺少安装版: $setupPath" }
  Copy-Item $setupPath $out
  Write-Host "  安装版 / setup:    $(Split-Path $setupPath -Leaf)"
}

Write-Host "--- 绿色版 / portable zips ---"
Pack-Portable "arm64" "src-tauri\target\release\listenforge.exe"                          "src-tauri\pdfium-arm64.dll"
Pack-Portable "x64"   "src-tauri\target\x86_64-pc-windows-msvc\release\listenforge.exe"   "src-tauri\pdfium-x64.dll"

Write-Host "`n--- 安装版 / setup exes ---"
Collect-Setup "src-tauri\target\release\bundle\nsis\listenforge_${Version}_arm64-setup.exe"
Collect-Setup "src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis\listenforge_${Version}_x64-setup.exe"

Write-Host "`n=== dist-release/ 最终产物 ==="
Get-ChildItem $out -File | ForEach-Object { "{0,-46} {1,7} MB" -f $_.Name, ([math]::Round($_.Length/1MB,2)) }
