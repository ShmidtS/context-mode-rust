@echo off
setlocal EnableDelayedExpansion

set "SCRIPT_DIR=%~dp0"

if exist "!SCRIPT_DIR!context-mode.exe" (
    "!SCRIPT_DIR!context-mode.exe" %*
    exit /b !ERRORLEVEL!
)

for %%t in (
    "!SCRIPT_DIR!..\..\target\release\context-mode.exe"
    "!SCRIPT_DIR!..\..\target\debug\context-mode.exe"
) do (
    if exist %%t (
        %%t %*
        exit /b !ERRORLEVEL!
    )
)

echo Error: context-mode binary not found.>&2
echo Run: cargo build --bin context-mode>&2
exit /b 1
