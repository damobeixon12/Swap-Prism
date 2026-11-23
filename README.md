Swap Prism

A visual customization and local asset management tool for Rocket League on Windows.

Swap Prism lets you configure cosmetic modifications through a unified interface: asset replacement, texture and audio libraries, profile customization, and Workshop map management.

Changes are processed locally through an operation queue, with backups and restore mechanisms. Customization modifies how the client displays content; it does not grant items to your account or change your server rank.

Features
Cosmetic customization: manage visual swaps for supported items.
Textures: import and organize decals, ball, and boost meter textures.
Audio: import MP3s, preview them, select the clip segment, and adjust volume for supported events.
Profile and titles: local customization of name, avatar, and titles.
Fake Rank: visual configuration of rank, division, and MMR, independent of your actual competitive ranking.
Workshop: map catalog with images, download, and import. Maps are applied exclusively on Utopia Retro / Wheel Ball.
FPS overlay: counter with configurable position, size, and color.
Change management: persistent queue, replacement of duplicate selections, and restoration of original assets.
Built-in updates: stable version detection, download with progress, SHA-256 verification, and installation with automatic restart.
Architecture

The application combines a native interface built with Rust and egui and an asset processing engine packaged in Python.

Operations run in the background using structured JSON communication. The engine includes compatibility and integrity checks, backups, and transactional application of changes.

Distribution uses an NSIS installer and a native update component. You do not need to install Rust or Python to use the application.

Installation and usage
Download the installer from GitHub Releases.
Install and open Swap Prism.
Check the Rocket League folder detected by the application.
Select your customizations and add them to Apply changes.
Close Rocket League before applying or restoring assets.
Open the game to check the result.

Updates preserve your existing selections, libraries, and backups.

Discord

Join the Swap Prism Discord to follow news, share configurations, and report bugs.

Join the Swap Prism Discord

Scope

Modifications are local, and their visibility depends on the asset and the game screen. Fake Rank does not alter matchmaking, rewards, or actual MMR.

Swap Prism is an independent project, not affiliated with Psyonix or Epic Games.
