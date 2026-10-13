<picture>
	<source media="(prefers-color-scheme: light)" srcset="../../.github/assets/app_cover_light.png">
	<img alt="Modrinth App Cover" src="../../.github/assets/app_cover.png">
</picture>

## Modrinth App

The Modrinth App is a desktop application for managing your Minecraft mods. It is built with [Tauri](https://tauri.app/) and [Vue](https://vuejs.org/).

If you're not a developer and you've stumbled upon this repository, you can download the latest release of the app from the [Modrinth website](https://modrinth.com/app).

## Development

### Pre-requisites

Before you begin, ensure you have the following installed on your machine:

- [Node.js](https://nodejs.org/en/)
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install)
- [Tauri](https://v2.tauri.app/start/prerequisites/)

### Setup

Follow these steps to set up your development environment:

```bash
pnpm install
pnpm app:dev
```

You should now have a development build of the app running with hot-reloading enabled. Any changes you make to the code will automatically refresh the app.

## Icarus updates

The `Icarus release` workflow builds signed bundles on GitHub-hosted Linux, Windows, and macOS runners. Set the repository secrets `TAURI_PRIVATE_KEY` and `TAURI_KEY_PASSWORD` to the private updater key and its passphrase. The matching public key is stored in `tauri-release.conf.json`.

Run the workflow manually to build and verify the bundles without publishing. To publish, push a `vMAJOR.MINOR.PATCH` tag matching the upstream app version. The workflow sets the app's build version from that tag, verifies every update signature, and publishes the installers and `latest.json` together. Prerelease tags publish prereleases without changing the stable updater endpoint.

Updates are served from [Icarus GitHub Releases](https://github.com/fraa2a/Icarus/releases). Windows bundles currently have an updater signature without an Authenticode certificate; macOS bundles are not notarized. A build containing the Icarus public key must be installed manually before it can accept Icarus updates.
