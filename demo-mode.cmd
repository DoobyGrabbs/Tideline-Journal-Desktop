@echo off
rem Starts Tideline Journal in demo mode: already unlocked, with made-up example entries, for showing the program
rem to someone. Everything it saves goes to a throw-away folder in Temp, so your real diary is never opened,
rem changed or shown. Close the window when you have finished.
set "HERE=%~dp0"
set "EXE=%HERE%target\release\tideline-journal.exe"
if not exist "%EXE%" set "EXE=%HERE%target\debug\tideline-journal.exe"
if not exist "%EXE%" (
    echo The program was not found. Build it first with: cargo build
    pause
    exit /b 1
)
set "TIDELINE_DEMO=1"
set "TIDELINE_DATA_DIR="
start "" "%EXE%"
