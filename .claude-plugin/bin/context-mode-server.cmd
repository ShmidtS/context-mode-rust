@echo off
setlocal EnableDelayedExpansion

set "SCRIPT_DIR=%~dp0"

if exist "!SCRIPT_DIR!context-mode-server.exe" (
    "!SCRIPT_DIR!context-mode-server.exe" %*
    exit /b !ERRORLEVEL!
)

for %%t in (
    "!SCRIPT_DIR!..\..\target\release\context-mode-server.exe"
    "!SCRIPT_DIR!..\..\target\debug\context-mode-server.exe"
) do (
    if exist %%t (
        %%t %*
        exit /b !ERRORLEVEL!
    )
)

echo Error: context-mode-server binary not found.>&2
echo Run: cargo build --bin context-mode-server>&2
exit /b 1
