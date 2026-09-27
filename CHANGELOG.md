# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) (before 1.0, a breaking
change or a Bevy / steamworks bump raises the minor version).

## [0.1.0] - Unreleased

First release, for Bevy 0.19.0 and steamworks 0.12.2.

### Added

- `SteamLobbyPlugin` (config `connect_prefix`, `set_connect_presence`, `check_launch_args`; copied
  into the `SteamLobbyConfig` resource): one Steam callback pump per frame in `First`, request
  handling in `Update`, leaving the lobby and clearing rich presence on `AppExit` in `Last`; the
  public `SteamLobbySystems::{Callbacks, Requests}` sets.
- Request messages `CreateLobby` (kind, member cap clamped to 250, lobby data), `JoinLobby`,
  `LeaveLobby`, `InviteFriend`, `SetRichPresence`, `ClearRichPresence`.
- Fact messages `LobbyCreated`, `LobbyEntered`, `JoinRequested` (with `JoinSource::LobbyInvite`,
  `RichPresence` or `LaunchArgs`), `LobbyLeft`, `InviteSent`, `LobbyError` with
  `LobbyErrorKind` (`#[non_exhaustive]`).
- The `SteamLobby` state resource (current lobby, requests in flight, a change counter).
- Rich presence `connect = "+connect_lobby <id>"` set automatically for a created lobby, so friends
  get "Join Game"; cold-launch detection of `+connect_lobby <id>` in the process arguments and in
  Steam's launch command line.
- Lobbies that complete after being abandoned (a leave or another join while in flight) are left
  at once and never adopted.
- The backend seam `SteamLobbyBackend` + `SteamLobbyBackendRes` + `BackendEvent`, with
  `RealSteamBackend` over steamworks 0.12.2 (feature `steam`; guards against the inputs that make
  steamworks panic) and the in-memory `FakeSteamBackend` + `FakeCall` for tests.
- Helpers `connect_string`, `parse_connect_lobby`, `is_individual_steam_id64`; the
  `MAX_LOBBY_MEMBERS` constant.
- Examples `quick_start` (fake backend), `host_lobby` and `join_lobby` (real Steam, feature
  `steam`); a README recipe for `bevy_replicon` 0.44 over `renet_steam` 3.0.0.
