@echo off
REM Build script for QuickRDP Tauri application
REM This script sets up the Visual Studio Build Tools environment and builds the app

echo Setting up Visual Studio Build Tools environment...
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64

if errorlevel 1 (
    echo ERROR: Failed to set up Visual Studio Build Tools environment
    echo Please ensure Visual Studio 2022 Build Tools is installed.
    pause
    exit /b 1
)

echo.
echo Building QuickRDP...
echo.

npm run tauri build

if errorlevel 1 (
    echo.
    echo ERROR: Build failed!
    pause
    exit /b 1
)

echo.
echo Build completed successfully!
echo Output files are in: src-tauri\target\release\bundle\
pause

