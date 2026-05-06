@echo off
setlocal enabledelayedexpansion

echo ============================================
echo Iris Hardening Web - Release Build
echo ============================================

:: 构建所有 crate
echo.
echo [1/4] Building iris-wasm-gateway...
cd crates\iris-wasm-gateway
cargo build --release
if errorlevel 1 goto error
cd ..\..

echo.
echo [2/4] Building iris-secure-gateway...
cd crates\iris-secure-gateway
cargo build --release
if errorlevel 1 goto error
cd ..\..

echo.
echo [3/4] Building iris-wasm...
cd crates\iris-wasm
call build-wasm.bat
if errorlevel 1 goto error
cd ..\..

echo.
echo [4/4] Copying artifacts...
if not exist release mkdir release
copy /Y crates\iris-wasm\pkg\iris.wasm release\
copy /Y crates\iris-wasm\pkg\init_iris.js release\
copy /Y target\release\iris-secure-gateway.exe release\
copy /Y target\release\iris-wasm-gateway.exe release\

echo.
echo ============================================
echo Build complete! Artifacts in release/
echo ============================================
goto end

:error
echo.
echo ERROR: Build failed!
exit /b 1

:end
endlocal
