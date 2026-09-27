//! The testable seam between the plugin's systems and Steam.
//!
//! The systems never touch `steamworks` directly: they talk to a [`SteamLobbyBackend`] stored in
//! [`SteamLobbyBackendRes`]. The real implementation is [`crate::RealSteamBackend`] (feature
//! `steam`); tests and non-Steam builds use [`crate::FakeSteamBackend`].

use bevy_ecs::prelude::Resource;

/// Steam lobby visibility (mirrors `steamworks::LobbyType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum LobbyKind {
    /// Only joinable by invitation.
    Private,
    /// Joinable by friends of members (and by invitation). The usual co-op choice.
    #[default]
    FriendsOnly,
    /// Listed publicly.
    Public,
    /// Joinable by anyone with the id, not listed; friends do not see it.
    Invisible,
}

/// How a join request reached this process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JoinSource {
    /// Steam `GameLobbyJoinRequested`: a friend clicked "Join Game" on a player in a lobby, or the
    /// user accepted a lobby invite, while this game was already running.
    LobbyInvite,
    /// Steam `GameRichPresenceJoinRequested`: the rich-presence `connect` string was used while the
    /// game was running.
    RichPresence,
    /// Cold launch: Steam started the process with the connect string on its command line.
    LaunchArgs,
}

/// A raw event produced by a backend's [`SteamLobbyBackend::pump`]. The plugin turns these into
/// its public messages (and applies its lobby bookkeeping) in [`crate::SteamLobbySystems::Callbacks`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendEvent {
    /// A `create_lobby` call completed successfully.
    LobbyCreated {
        /// The new lobby's raw id.
        lobby: u64,
    },
    /// A `create_lobby` call failed.
    LobbyCreateFailed {
        /// Human-readable reason from Steam.
        message: String,
    },
    /// A `join_lobby` call completed successfully.
    LobbyEntered {
        /// The joined lobby's raw id.
        lobby: u64,
    },
    /// A `join_lobby` call failed.
    LobbyJoinFailed {
        /// The lobby that could not be joined.
        lobby: u64,
    },
    /// Steam `GameLobbyJoinRequested` (lobby invite accepted / "Join Game" on a friend).
    LobbyJoinRequested {
        /// The lobby to join.
        lobby: u64,
        /// The friend it came from (raw SteamID64).
        from: u64,
    },
    /// Steam `GameRichPresenceJoinRequested`: the raw `connect` string, parsed by the plugin.
    RichPresenceJoinRequested {
        /// The friend it came from (raw SteamID64; may be invalid when not from a friend).
        from: u64,
        /// The rich-presence connect string.
        connect: String,
    },
}

/// Everything the plugin needs from Steam. Implementations must never panic; a failure is a
/// `false` / `None` / an error event.
///
/// Asynchronous calls (`create_lobby`, `join_lobby`) return nothing: their outcome is queued and
/// returned by a later [`pump`](Self::pump).
pub trait SteamLobbyBackend: Send + Sync + 'static {
    /// The local user's SteamID64.
    fn local_id(&self) -> u64;
    /// Start creating a lobby. Outcome: [`BackendEvent::LobbyCreated`] or
    /// [`BackendEvent::LobbyCreateFailed`] from a later `pump`.
    fn create_lobby(&self, kind: LobbyKind, max_members: u32);
    /// Start joining a lobby. Outcome: [`BackendEvent::LobbyEntered`] or
    /// [`BackendEvent::LobbyJoinFailed`] from a later `pump`.
    fn join_lobby(&self, lobby: u64);
    /// Leave a lobby (no-op if not a member).
    fn leave_lobby(&self, lobby: u64);
    /// Set a lobby metadata key (owner only). `false` on failure.
    fn set_lobby_data(&self, lobby: u64, key: &str, value: &str) -> bool;
    /// Read a lobby metadata key. `None` when missing.
    fn lobby_data(&self, lobby: u64, key: &str) -> Option<String>;
    /// Number of members currently in the lobby.
    fn lobby_member_count(&self, lobby: u64) -> usize;
    /// Allow or forbid joining the lobby. `false` on failure.
    fn set_lobby_joinable(&self, lobby: u64, joinable: bool) -> bool;
    /// Set (`Some`) or remove (`None`) one rich-presence key. `false` on failure.
    fn set_rich_presence(&self, key: &str, value: Option<&str>) -> bool;
    /// Remove every rich-presence key this process set.
    fn clear_rich_presence(&self);
    /// A user's persona name ("" when unknown).
    fn friend_name(&self, id: u64) -> String;
    /// Send a Steam game invite carrying `connect` to `friend`. `false` if it could not be sent.
    fn invite_to_game(&self, friend: u64, connect: &str) -> bool;
    /// The command line Steam launched the game with ("" when none).
    fn launch_command_line(&self) -> String;
    /// Pump Steam callbacks once and return everything that arrived since the last pump
    /// (callbacks + completed create/join call results). Called exactly once per frame.
    fn pump(&self) -> Vec<BackendEvent>;
}

/// The active backend. Insert it to make the plugin live; without it every system is inert and
/// requests are answered with [`crate::LobbyErrorKind::NoBackend`].
#[derive(Resource)]
pub struct SteamLobbyBackendRes(pub Box<dyn SteamLobbyBackend>);
