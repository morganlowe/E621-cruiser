@echo off
setlocal
cd /d "%~dp0"

set "BIN=%~dp0target\release\e621-cruiser.exe"
if not exist "%BIN%" (
    where cargo >nul 2>nul
    if %errorlevel% neq 0 (
        echo Error: Cargo ^(Rust toolchain^) was not found in PATH.
        echo Please install Rust from https://rustup.rs/
        pause
        exit /b 1
    )
    echo Building E621 Cruiser...
    cargo build --release
    if %errorlevel% neq 0 (
        echo Build failed.
        pause
        exit /b 1
    )
)

start "" "%BIN%" %*
