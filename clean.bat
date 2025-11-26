@echo off
REM QuickRDP Build Artifact Cleanup Script
REM This script removes build artifacts and temporary files while preserving source code

echo ====================================
echo QuickRDP Cleanup Script
echo ====================================
echo.

REM Change to the project root directory
cd /d "%~dp0"

echo [1/11] Cleaning Node.js build outputs...
if exist "dist" (
    rmdir /s /q "dist" 2>nul
    if not exist "dist" (
        echo   - Removed dist/
    ) else (
        echo   - Warning: Could not remove dist/
    )
)
if exist "dist-ssr" (
    rmdir /s /q "dist-ssr" 2>nul
    if not exist "dist-ssr" (
        echo   - Removed dist-ssr/
    ) else (
        echo   - Warning: Could not remove dist-ssr/
    )
)

echo [2/11] Cleaning Vite cache...
if exist ".vite" (
    rmdir /s /q ".vite" 2>nul
    echo   - Removed .vite/
)
if exist ".vite-inspect" (
    rmdir /s /q ".vite-inspect" 2>nul
    echo   - Removed .vite-inspect/
)
del /q "vite.config.*.timestamp-*" 2>nul

echo [3/11] Cleaning Rust/Tauri build artifacts...
if exist "src-tauri\target\debug" (
    rmdir /s /q "src-tauri\target\debug" 2>nul
    echo   - Removed src-tauri\target\debug/
)
if exist "src-tauri\target\release" (
    rmdir /s /q "src-tauri\target\release" 2>nul
    echo   - Removed src-tauri\target\release/
)
if exist "src-tauri\target\flycheck0" (
    rmdir /s /q "src-tauri\target\flycheck0" 2>nul
    echo   - Removed src-tauri\target\flycheck0/
)
if exist "src-tauri\target\.rustc_info.json" (
    del /q "src-tauri\target\.rustc_info.json" 2>nul
    echo   - Removed src-tauri\target\.rustc_info.json
)
if exist "src-tauri\target\CACHEDIR.TAG" (
    del /q "src-tauri\target\CACHEDIR.TAG" 2>nul
    echo   - Removed src-tauri\target\CACHEDIR.TAG
)

echo [4/11] Cleaning Rust incremental compilation artifacts...
for /d /r "src-tauri\target" %%d in (incremental) do (
    if exist "%%d" (
        rmdir /s /q "%%d" 2>nul
        echo   - Removed %%d
    )
)
del /s /q "src-tauri\src\*.rs.bk" 2>nul

echo [5/11] Cleaning Tauri generated files...
if exist "src-tauri\gen" (
    rmdir /s /q "src-tauri\gen" 2>nul
    echo   - Removed src-tauri\gen/
)

echo [6/11] Cleaning WiX Toolset temporary files...
if exist "src-tauri\WixTools" (
    rmdir /s /q "src-tauri\WixTools" 2>nul
    echo   - Removed src-tauri\WixTools/
)
del /s /q "*.wixobj" 2>nul
del /s /q "*.wixpdb" 2>nul
echo   - Removed WiX temporary files

echo [7/11] Cleaning installer artifacts...
del /q "*.msi" 2>nul
del /q "src-tauri\target\release\*.exe" 2>nul
echo   - Removed installer files (*.msi, *.exe)

echo [8/11] Cleaning log files...
del /s /q "*.log" 2>nul
if exist "logs" (
    rmdir /s /q "logs" 2>nul
    echo   - Removed logs/
)
echo   - Removed *.log files

echo [9/11] Cleaning TypeScript build info...
del /s /q "*.tsbuildinfo" 2>nul
echo   - Removed *.tsbuildinfo files

echo [10/11] Cleaning cache directories...
if exist ".cache" (
    rmdir /s /q ".cache" 2>nul
    echo   - Removed .cache/
)
if exist ".eslintcache" (
    del /q ".eslintcache" 2>nul
    echo   - Removed .eslintcache
)
if exist ".stylelintcache" (
    del /q ".stylelintcache" 2>nul
    echo   - Removed .stylelintcache
)

echo [11/11] Cleaning temporary and debug files...
del /q "*.tmp" 2>nul
del /q "*.temp" 2>nul
del /q "*.swp" 2>nul
del /q "*.swo" 2>nul
del /q "*~" 2>nul
del /q "*.pdb" 2>nul
del /q "*.dmp" 2>nul
echo   - Removed temporary files (*.tmp, *.temp, *.swp, etc.)
if exist "debug_output.txt" (
    del /q "debug_output.txt" 2>nul
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
echo   - Icons and static assets
echo.
echo To do a complete clean rebuild:
echo   1. Run this script
echo   2. Delete node_modules/ manually if needed
echo   3. Run 'npm install' to reinstall dependencies
echo   4. Run 'build.bat' to rebuild the application
echo.

pause
