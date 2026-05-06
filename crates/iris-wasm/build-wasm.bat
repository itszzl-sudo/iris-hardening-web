@echo off
echo Building iris-wasm...
cd /d "%~dp0"
wasm-pack build --target web --release --no-opt

echo Renaming WASM file...
if exist pkg\iris_wasm_bg.wasm (
    move /Y pkg\iris_wasm_bg.wasm pkg\iris.wasm
    echo Renamed iris_wasm_bg.wasm to iris.wasm
)

echo Done! WASM module built to pkg/
pause
pause
