@echo off
setlocal EnableDelayedExpansion
cd /d "%~dp0"

echo ==========================================================
echo   E621 Cruiser - Windows Installer
echo ==========================================================
echo.

where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] Cargo ^(Rust toolchain^) was not found in PATH.
    echo Please install Rust from https://rustup.rs/
    echo.
    pause
    exit /b 1
)

echo [1/2] Building E621 Cruiser (release)...
cargo build --release
if %errorlevel% neq 0 (
    echo [ERROR] Build failed.
    pause
    exit /b 1
)

echo [2/2] Creating Windows Desktop and Start Menu shortcuts...
set "SCRIPT_DIR=%~dp0"
if "%SCRIPT_DIR:~-1%"=="\" set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"

set "TARGET_EXE=%SCRIPT_DIR%\target\release\e621-cruiser.exe"
set "ICON_FILE=%SCRIPT_DIR%\assets\e621-cruiser.ico"
set "DESKTOP_LNK=%USERPROFILE%\Desktop\E621 Cruiser.lnk"
set "START_MENU_DIR=%APPDATA%\Microsoft\Windows\Start Menu\Programs"
set "START_LNK=%START_MENU_DIR%\E621 Cruiser.lnk"

powershell -NoProfile -ExecutionPolicy Bypass -Command ^
    "$ws = New-Object -ComObject WScript.Shell; " ^
    "$s1 = $ws.CreateShortcut('%DESKTOP_LNK%'); " ^
    "$s1.TargetPath = '%TARGET_EXE%'; " ^
    "$s1.WorkingDirectory = '%SCRIPT_DIR%'; " ^
    "$s1.IconLocation = '%ICON_FILE%,0'; " ^
    "$s1.Description = 'Browse and download posts, pools, and favorites from e621'; " ^
    "$s1.Save(); " ^
    "$s2 = $ws.CreateShortcut('%START_LNK%'); " ^
    "$s2.TargetPath = '%TARGET_EXE%'; " ^
    "$s2.WorkingDirectory = '%SCRIPT_DIR%'; " ^
    "$s2.IconLocation = '%ICON_FILE%,0'; " ^
    "$s2.Description = 'Browse and download posts, pools, and favorites from e621'; " ^
    "$s2.Save();"

echo.
echo ==========================================================
echo   Installation complete!
echo   * Desktop shortcut created:  Desktop\E621 Cruiser.lnk
echo   * Start menu entry added:    Start Menu \ E621 Cruiser
echo   * Executable:                target\release\e621-cruiser.exe
echo ==========================================================
echo.
pause
