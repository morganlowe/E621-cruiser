@echo off
setlocal EnableDelayedExpansion
cd /d "%~dp0"

echo ==========================================================
echo   E621 Cruiser - Windows Installer
echo ==========================================================
echo.

where python >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] Python 3 was not found in PATH.
    echo Please install Python 3.10+ from https://www.python.org/
    echo Make sure to check "Add Python to PATH" during installation.
    echo.
    pause
    exit /b 1
)

echo [1/3] Setting up Python virtual environment...
if not exist ".venv\Scripts\python.exe" (
    python -m venv .venv
)

echo [2/3] Installing dependencies...
.venv\Scripts\python.exe -m pip install --upgrade pip
.venv\Scripts\pip.exe install -r requirements.txt

echo [3/3] Creating Windows Desktop and Start Menu shortcuts...
set "SCRIPT_DIR=%~dp0"
:: Strip trailing backslash
if "%SCRIPT_DIR:~-1%"=="\" set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"

set "TARGET_RUN=%SCRIPT_DIR%\run.bat"
set "ICON_FILE=%SCRIPT_DIR%\assets\e621-cruiser.ico"
set "DESKTOP_LNK=%USERPROFILE%\Desktop\E621 Cruiser.lnk"
set "START_MENU_DIR=%APPDATA%\Microsoft\Windows\Start Menu\Programs"
set "START_LNK=%START_MENU_DIR%\E621 Cruiser.lnk"

:: Use PowerShell to create the .lnk shortcuts
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
    "$ws = New-Object -ComObject WScript.Shell; " ^
    "$s1 = $ws.CreateShortcut('%DESKTOP_LNK%'); " ^
    "$s1.TargetPath = '%TARGET_RUN%'; " ^
    "$s1.WorkingDirectory = '%SCRIPT_DIR%'; " ^
    "$s1.IconLocation = '%ICON_FILE%,0'; " ^
    "$s1.Description = 'Browse and download posts, pools, and favorites from e621'; " ^
    "$s1.Save(); " ^
    "$s2 = $ws.CreateShortcut('%START_LNK%'); " ^
    "$s2.TargetPath = '%TARGET_RUN%'; " ^
    "$s2.WorkingDirectory = '%SCRIPT_DIR%'; " ^
    "$s2.IconLocation = '%ICON_FILE%,0'; " ^
    "$s2.Description = 'Browse and download posts, pools, and favorites from e621'; " ^
    "$s2.Save();"

echo.
echo ==========================================================
echo   Installation complete!
echo   * Desktop shortcut created:  Desktop\E621 Cruiser.lnk
echo   * Start menu entry added:    Start Menu \ E621 Cruiser
echo   * Direct launcher:           run.bat
echo ==========================================================
echo.
pause
