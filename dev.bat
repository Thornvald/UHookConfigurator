@echo off
cd /d "%~dp0"
for %%P in (1420 1421) do (
  for /f "tokens=5" %%A in ('netstat -ano ^| findstr ":%%P " ^| findstr LISTENING') do (
    taskkill /PID %%A /F >nul 2>&1
  )
)
echo Starting Tauri Dev...
echo Tauri will start the Bun dev server automatically.
cargo tauri dev
if errorlevel 1 pause
