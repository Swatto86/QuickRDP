@echo off
REM QuickRDP Build Artifact Cleanup Script
REM This script removes build artifacts and temporary files while preserving source code

echo ====================================
echo QuickRDP Cleanup Script
echo ====================================
echo.

REM Change to the project root directory
cd /d "%~dp0"

echo [1/8] Cleaning Node.js build outputs...
if exist "dist" (
    rmdir /s /q "dist"
    echo   - Removed dist/
)
if exist "dist-ssr" (
    rmdir /s /q "dist-ssr"
    echo   - Removed dist-ssr/
)

echo [2/8] Cleaning Rust/Tauri build artifacts...
if exist "src-tauri\target\debug" (
    rmdir /s /q "src-tauri\target\debug"
    echo   - Removed src-tauri\target\debug/
)
if exist "src-tauri\target\release" (
    rmdir /s /q "src-tauri\target\release"
    echo   - Removed src-tauri\target\release/
)
if exist "src-tauri\target\flycheck0" (
    rmdir /s /q "src-tauri\target\flycheck0"
    echo   - Removed src-tauri\target\flycheck0/
)
if exist "src-tauri\target\CACHEDIR.TAG" (
    del /q "src-tauri\target\CACHEDIR.TAG"
    echo   - Removed src-tauri\target\CACHEDIR.TAG
)

echo [3/8] Cleaning Tauri generated files...
if exist "src-tauri\gen" (
    rmdir /s /q "src-tauri\gen"
    echo   - Removed src-tauri\gen/
)

echo [4/8] Cleaning log files...
del /s /q "*.log" 2>nul
if exist "logs" (
    rmdir /s /q "logs"
    echo   - Removed logs/
)
echo   - Removed *.log files

echo [5/8] Cleaning TypeScript build info...
del /s /q "*.tsbuildinfo" 2>nul
echo   - Removed *.tsbuildinfo files

echo [6/8] Cleaning cache directories...
if exist ".cache" (
    rmdir /s /q ".cache"
    echo   - Removed .cache/
)
if exist ".eslintcache" (
    del /q ".eslintcache"
    echo   - Removed .eslintcache
)
if exist ".stylelintcache" (
    del /q ".stylelintcache"
    echo   - Removed .stylelintcache
)

echo [7/8] Cleaning temporary files...
del /q "*.tmp" 2>nul
del /q "*.temp" 2>nul
del /q "*.swp" 2>nul
del /q "*.swo" 2>nul
del /q "*~" 2>nul
echo   - Removed temporary files (*.tmp, *.temp, *.swp, etc.)

echo [8/8] Cleaning debug output files...
if exist "debug_output.txt" (
    del /q "debug_output.txt"
    echo   - Removed debug_output.txt
)

echo.
echo ====================================
echo Cleanup Complete!
echo ====================================
echo.
echo The following were preserved:
echo   - Source code (src/, src-tauri/src/)
echo   - Configuration files
echo   - node_modules/ (use 'npm clean-install' to reinstall if needed)
echo   - Cargo registry cache (src-tauri/target/.rustc_info.json)
echo.

pause
