@echo off
setlocal
cd /d "%~dp0"

:: Check for Python virtual environment
if not exist ".venv\Scripts\python.exe" (
    echo Setting up virtual environment for E621 Cruiser...
    where python >nul 2>nul
    if %errorlevel% neq 0 (
        echo Error: Python 3 was not found in PATH.
        echo Please install Python 3.10 or newer from https://www.python.org/
        pause
        exit /b 1
    )
    python -m venv .venv
    echo Installing dependencies...
    .venv\Scripts\pip.exe install -r requirements.txt
)

:: Launch with pythonw (no lingering console window)
if exist ".venv\Scripts\pythonw.exe" (
    start "" ".venv\Scripts\pythonw.exe" -m e621cruiser %*
) else (
    start "" ".venv\Scripts\python.exe" -m e621cruiser %*
)
