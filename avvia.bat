@echo off
setlocal EnableDelayedExpansion
title NovaCraft Launcher - Avvio

echo ===================================================
echo             NovaCraft Launcher - Avvio
echo ===================================================
echo.

cd /d "%~dp0"

:: 1. Verifica presenza di Node.js
where node >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERRORE] Node.js non e' installato o non e' presente nel PATH.
    echo Scarica e installa Node.js da: https://nodejs.org/
    echo.
    pause
    exit /b 1
)

:: 2. Verifica se le dipendenze npm sono installate
if not exist "node_modules\" (
    echo [INFO] Cartella node_modules non trovata.
    echo [INFO] Installazione delle dipendenze in corso con 'npm install'...
    call npm install
    if %errorlevel% neq 0 (
        echo [ERRORE] Errore durante l'installazione delle dipendenze npm.
        echo.
        pause
        exit /b 1
    )
)

:: 3. Avvio dell'applicazione con Tauri
echo [INFO] Avvio di NovaCraft Launcher in corso...
echo [INFO] Apertura del server Vite e della finestra Tauri...
echo.

call npm run tauri dev

if %errorlevel% neq 0 (
    echo.
    echo [ERRORE] L'applicazione si e' chiusa con un errore (codice: %errorlevel%).
    echo.
    pause
    exit /b %errorlevel%
)

exit /b 0
