# NovaCraft Launcher

Launcher per Minecraft, leggero e moderno, fatto con **Tauri v2** (backend in Rust) e un'interfaccia in HTML/CSS/JavaScript (Vite).

## Cosa sa fare

- Scarica e avvia qualsiasi versione di Minecraft (release, snapshot e versioni storiche), anche **offline** se la versione è già installata.
- Scarica in automatico il **Java giusto** per ogni versione (Adoptium Temurin 8, 17, 21, 25…).
- **Fabric** con un clic.
- Browser di **mod, shader e resource pack** da Modrinth: installa da solo le dipendenze (es. Fabric API) e, per gli shader, Iris + Sodium.
- **Profili** con cartelle separate (mod, mondi e impostazioni indipendenti).
- **Skin personalizzata**: visibile nel launcher e anche in gioco (in giocatore singolo).
- Console con i log del gioco e messaggi chiari quando Minecraft va in crash.

> Il gioco parte in modalità offline: i server "premium" (online-mode) non sono ancora supportati.

## Avvio per lo sviluppo

Serve [Node.js](https://nodejs.org/) e [Rust](https://rustup.rs/).

- Doppio clic su `avvia.bat` (installa le dipendenze la prima volta e apre il launcher), oppure:

```bash
npm install
npm run tauri dev
```

## Creare l'eseguibile

- Doppio clic su `compila-release.bat`: i file pronti finiscono in `NovaCraft-Release\`.
- La versione per Mac viene compilata da GitHub Actions (`.github/workflows/build.yml`) a ogni push su `main`.

## Dove salva i file

Tutto in `%APPDATA%\NovaCraft` (Windows) o `~/Library/Application Support/NovaCraft` (Mac):
`versions`, `libraries`, `assets` sono in comune; ogni profilo extra ha la sua cartella in `instances\`.
