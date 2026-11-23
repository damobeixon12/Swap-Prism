<div align="center">

# Swap Prism

**Visual customization and local asset management for Rocket League on Windows**

[![Platform](https://img.shields.io/badge/platform-Windows-0078D6?logo=windows&logoColor=white)](#installation)
[![Built with Rust](https://img.shields.io/badge/UI-Rust%20%2B%20egui-000000?logo=rust&logoColor=white)](#architecture)
[![Engine](https://img.shields.io/badge/engine-Python-3776AB?logo=python&logoColor=white)](#architecture)
[![Download](https://img.shields.io/badge/download-latest%20release-2ea44f)](https://github.com/damobeixon12/Swap-Prism/releases/latest)
[![Discord](https://img.shields.io/badge/Discord-join%20the%20community-5865F2?logo=discord&logoColor=white)](https://discord.gg/dXa24Y5HtX)

</div>

---

## Overview

Swap Prism is a desktop application that lets you configure cosmetic modifications for Rocket League through a single, unified interface. It covers asset replacement, texture and audio libraries, profile customization, and Workshop map management.

All changes are processed locally through an operation queue, with backups and restore mechanisms built in. Swap Prism modifies how the game client presents content. It does **not** grant items to your account and does **not** affect your server-side rank.

## Table of Contents

- [Features](#features)
- [Architecture](#architecture)
- [Installation](#installation)
- [Usage](#usage)
- [Scope and Limitations](#scope-and-limitations)
- [Community](#community)
- [Disclaimer](#disclaimer)

## Features

| Feature | Description |
| --- | --- |
| **Cosmetic customization** | Manage visual swaps for supported items. |
| **Textures** | Import and organize decal, ball, and boost meter textures. |
| **Audio** | Import MP3 files, preview them, select the clip segment, and adjust volume for supported in-game events. |
| **Profile and titles** | Locally customize your name, avatar, and titles. |
| **Fake Rank** | Configure a visual rank, division, and MMR, fully independent of your real competitive ranking. |
| **Workshop** | Browse a map catalog with preview images, then download and import maps. Maps are applied exclusively to Utopia Retro / Wheel Ball. |
| **FPS overlay** | On-screen counter with configurable position, size, and color. |
| **Change management** | Persistent queue, automatic replacement of duplicate selections, and restoration of original assets. |
| **Built-in updates** | Stable-release detection, progress-tracked downloads, SHA-256 verification, and installation with automatic restart. |

## Architecture

Swap Prism combines two components:

- **Native interface:** built with Rust and [egui](https://github.com/emilk/egui).
- **Asset processing engine:** packaged with Python.

Operations run in the background and communicate through structured JSON messages. The engine performs compatibility and integrity checks, creates backups, and applies changes transactionally.

The application is distributed with an NSIS installer and a native update component. **Neither Rust nor Python needs to be installed** to use it.

## Installation

1. Download the installer from the [latest GitHub release](https://github.com/damobeixon12/Swap-Prism/releases/latest).
2. Run the installer and launch Swap Prism.

Updates preserve your existing selections, libraries, and backups.

## Usage

1. Open Swap Prism and verify the Rocket League installation folder detected by the application.
2. Select your desired customizations and add them to **Apply changes**.
3. **Close Rocket League** before applying or restoring assets.
4. Apply the changes.
5. Launch the game to verify the result.

> [!IMPORTANT]
> Rocket League must be closed whenever you apply or restore assets.

## Scope and Limitations

- All modifications are local. Their visibility depends on the asset type and the in-game screen.
- Fake Rank does not alter matchmaking, rewards, or your actual MMR.
- Workshop maps are applied only to Utopia Retro / Wheel Ball.

## Community

Join the Discord server to follow announcements, share configurations, and report bugs.

[**Join the Swap Prism Discord →**](https://discord.gg/dXa24Y5HtX)

## Disclaimer

Swap Prism is an independent project and is not affiliated with, endorsed by, or sponsored by Psyonix or Epic Games.
