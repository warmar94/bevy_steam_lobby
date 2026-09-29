# bevy_steam_lobby (archived)

> **This crate has moved to [`bevy_steam_kit`](https://github.com/warmar94/bevy_steam_kit).**
> It has been removed from crates.io, and this repository is archived (read-only).

## Why

`bevy_steam_lobby` did one thing: Steam lobbies, invites and rich presence. It is now the `lobby`
feature of **`bevy_steam_kit`**, a modular Steam kit for Bevy that also adds achievements + stats
(`stats`) and leaderboards (`leaderboards`), each opt-in.

Steam needs exactly **one** callback pump per game. In this crate the lobby plugin owned it, so a
second Steam crate (achievements, leaderboards) could not have its own without breaking things.
The kit moves the pump into a shared core that every feature plugs into; doing that as separate
crates would have meant releasing several crates in lockstep anyway (they all share the same Bevy
and steamworks pins), so it became one crate with features.

## Migrating

```toml
# before
bevy_steam_lobby = { version = "0.1", features = ["steam"] }
# after
bevy_steam_kit = { version = "0.1", features = ["lobby", "steam"] }
```

Every lobby message and type keeps its name (`CreateLobby`, `JoinLobby`, `JoinRequested`,
`LobbyEntered`, ...). What changes: the import path, the plugin line
(`SteamKitPlugin::default().with_lobby(LobbySettings { .. })`), the backend resource
(`SteamBackendRes`), and the system sets (`SteamKitSystems`). The full table is in the kit's README,
section "Migrating from bevy_steam_lobby".

## License

MIT OR Apache-2.0, as before.
