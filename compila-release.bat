@echo off
setlocal EnableDelayedExpansion
title NovaCraft Launcher - Compilazione Release

echo ===================================================
echo     NovaCraft Launcher - Generazione .EXE Release
echo ===================================================
echo.

cd /d "%~dp0"

echo [1/3] Compilazione in corso con Tauri...
call npm run tauri build

if %errorlevel% neq 0 (
    echo.
    echo [ERRORE] La compilazione e' fallita!
    echo.
    pause
    exit /b %errorlevel%
)

echo [2/3] Creazione cartella NovaCraft-Release...
if not exist "NovaCraft-Release\" mkdir "NovaCraft-Release"

echo [3/3] Copia dei file generati in NovaCraft-Release...
copy /Y "src-tauri\target\release\novacraft-launcher.exe" "NovaCraft-Release\NovaCraft-Launcher.exe" >nul
copy /Y "src-tauri\target\release\bundle\nsis\NovaCraft Launcher_0.1.0_x64-setup.exe" "NovaCraft-Release\NovaCraft-Launcher-Setup.exe" >nul

echo.
echo ===================================================
echo   Compilazione completata con successo!
echo   I file pronti da inviare si trovano in:
echo   %~dp0NovaCraft-Release\
echo ===================================================
echo.
pause
