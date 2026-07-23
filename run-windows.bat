@echo off
REM One-click launcher for DualSense Haptics on Windows.
REM Sets PATH so node + cargo are always found, kills stale instances, then runs dev.

set "PATH=%PATH%;C:\Program Files\nodejs;%USERPROFILE%\.cargo\bin"

echo Killing any stale instances...
taskkill /F /IM dualsense-haptics.exe >nul 2>&1

cd /d "%~dp0"

echo Starting DualSense Haptics (first build may take a minute)...
call npm run dev

echo.
echo App exited. Press any key to close.
pause >nul
