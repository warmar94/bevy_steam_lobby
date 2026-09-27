//! [`FakeSteamBackend`]: an in-memory Steam for tests and for driving the plugin without Steam.
//! It never touches the network or the Steam client.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::backend::{BackendEvent, LobbyKind, SteamLobbyBackend};

/// One recorded call into the fake backend (queries such as `lobby_data` are not recorded).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FakeCall {
    /// `create_lobby(kind, max_members)`.
    CreateLobby {
        /// Requested visibility.
        kind: LobbyKind,
        /// Requested member cap.
        max_members: u32,
    },
    /// `join_lobby(lobby)`.
    JoinLobby(u64),
    /// `leave_lobby(lobby)`.
    LeaveLobby(u64),
    /// `set_lobby_data(lobby, key, value)`.
    SetLobbyData {
        /// Lobby.
        lobby: u64,
        /// Key.
        key: String,
        /// Value.
        value: String,
    },
    /// `set_lobby_joinable(lobby, joinable)`.
    SetLobbyJoinable {
        /// Lobby.
        lobby: u64,
        /// Joinable flag.
        joinable: bool,
    },
    /// `set_rich_presence(key, value)`.
    SetRichPresence {
        /// Key.
        key: String,
        /// Value (`None` = remove).
        value: Option<String>,
    },
    /// `clear_rich_presence()`.
    ClearRichPresence,
    /// `invite_to_game(friend, connect)`.
    InviteToGame {
        /// Invited user.
        friend: u64,
        /// Connect string sent.
        connect: String,
    },
}

#[derive(Debug)]
struct FakeState {
    local_id: u64,
    calls: Vec<FakeCall>,
    queued: Vec<BackendEvent>,
    auto_complete_create: bool,
    join_succeeds: bool,
    next_lobby: u64,
    lobby_data: HashMap<(u64, String), String>,
    member_counts: HashMap<u64, usize>,
    rich_presence: HashMap<String, String>,
    friend_names: HashMap<u64, String>,
    launch_command_line: String,
}

/// An in-memory Steam. Cheap to clone: every clone shares the same state, so a test keeps one
/// clone to inspect while the plugin owns another inside [`crate::SteamLobbyBackendRes`].
///
/// Defaults: `create_lobby` completes on the next pump with ids `1000, 1001, ...`; `join_lobby`
/// succeeds on the next pump; local id `76561197960265729`.
#[derive(Clone, Debug)]
pub struct FakeSteamBackend {
    state: Arc<Mutex<FakeState>>,
}

impl Default for FakeSteamBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeSteamBackend {
    /// A fresh fake Steam with the defaults described on the type.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                local_id: 76_561_197_960_265_729,
                calls: Vec::new(),
                queued: Vec::new(),
                auto_complete_create: true,
                join_succeeds: true,
                next_lobby: 1000,
                lobby_data: HashMap::new(),
                member_counts: HashMap::new(),
                rich_presence: HashMap::new(),
                friend_names: HashMap::new(),
                launch_command_line: String::new(),
            })),
        }
    }

    fn lock(&self) -> MutexGuard<'_, FakeState> {
        // A poisoned mutex only means a test thread panicked mid-call; the data is still usable.
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Every call recorded so far, in order.
    pub fn calls(&self) -> Vec<FakeCall> {
        self.lock().calls.clone()
    }

    /// Set the local user's SteamID64.
    pub fn set_local_id(&self, id: u64) {
        self.lock().local_id = id;
    }

    /// `false`: `create_lobby` stays pending until [`complete_create`](Self::complete_create) /
    /// [`fail_create`](Self::fail_create).
    pub fn set_auto_complete_create(&self, auto: bool) {
        self.lock().auto_complete_create = auto;
    }

    /// Make future `join_lobby` calls fail (`false`) or succeed (`true`).
    pub fn set_join_succeeds(&self, ok: bool) {
        self.lock().join_succeeds = ok;
    }

    /// Complete a pending create with `lobby` (delivered on the next pump).
    pub fn complete_create(&self, lobby: u64) {
        self.lock().queued.push(BackendEvent::LobbyCreated { lobby });
    }

    /// Fail a pending create (delivered on the next pump).
    pub fn fail_create(&self, message: &str) {
        self.lock().queued.push(BackendEvent::LobbyCreateFailed { message: message.to_string() });
    }

    /// Queue any raw backend event (e.g. a join request) for the next pump.
    pub fn push_event(&self, event: BackendEvent) {
        self.lock().queued.push(event);
    }

    /// Set a lobby's member count as seen by `lobby_member_count`.
    pub fn set_member_count(&self, lobby: u64, count: usize) {
        self.lock().member_counts.insert(lobby, count);
    }

    /// Pre-set lobby metadata (as if another host had set it).
    pub fn put_lobby_data(&self, lobby: u64, key: &str, value: &str) {
        self.lock().lobby_data.insert((lobby, key.to_string()), value.to_string());
    }

    /// Current rich presence value for `key`.
    pub fn rich_presence(&self, key: &str) -> Option<String> {
        self.lock().rich_presence.get(key).cloned()
    }

    /// Give a user a persona name.
    pub fn set_friend_name(&self, id: u64, name: &str) {
        self.lock().friend_names.insert(id, name.to_string());
    }

    /// Set what `launch_command_line` returns.
    pub fn set_launch_command_line(&self, text: &str) {
        self.lock().launch_command_line = text.to_string();
    }
}

impl SteamLobbyBackend for FakeSteamBackend {
    fn local_id(&self) -> u64 {
        self.lock().local_id
    }

    fn create_lobby(&self, kind: LobbyKind, max_members: u32) {
        let mut s = self.lock();
        s.calls.push(FakeCall::CreateLobby { kind, max_members });
        if s.auto_complete_create {
            let lobby = s.next_lobby;
            s.next_lobby += 1;
            s.member_counts.insert(lobby, 1);
            s.queued.push(BackendEvent::LobbyCreated { lobby });
        }
    }

    fn join_lobby(&self, lobby: u64) {
        let mut s = self.lock();
        s.calls.push(FakeCall::JoinLobby(lobby));
        let ev = if s.join_succeeds { BackendEvent::LobbyEntered { lobby } } else { BackendEvent::LobbyJoinFailed { lobby } };
        s.queued.push(ev);
    }

    fn leave_lobby(&self, lobby: u64) {
        self.lock().calls.push(FakeCall::LeaveLobby(lobby));
    }

    fn set_lobby_data(&self, lobby: u64, key: &str, value: &str) -> bool {
        let mut s = self.lock();
        s.calls.push(FakeCall::SetLobbyData { lobby, key: key.to_string(), value: value.to_string() });
        s.lobby_data.insert((lobby, key.to_string()), value.to_string());
        true
    }

    fn lobby_data(&self, lobby: u64, key: &str) -> Option<String> {
        self.lock().lobby_data.get(&(lobby, key.to_string())).cloned()
    }

    fn lobby_member_count(&self, lobby: u64) -> usize {
        self.lock().member_counts.get(&lobby).copied().unwrap_or(0)
    }

    fn set_lobby_joinable(&self, lobby: u64, joinable: bool) -> bool {
        self.lock().calls.push(FakeCall::SetLobbyJoinable { lobby, joinable });
        true
    }

    fn set_rich_presence(&self, key: &str, value: Option<&str>) -> bool {
        let mut s = self.lock();
        s.calls.push(FakeCall::SetRichPresence { key: key.to_string(), value: value.map(str::to_string) });
        match value {
            Some(v) => s.rich_presence.insert(key.to_string(), v.to_string()),
            None => s.rich_presence.remove(key),
        };
        true
    }

    fn clear_rich_presence(&self) {
        let mut s = self.lock();
        s.calls.push(FakeCall::ClearRichPresence);
        s.rich_presence.clear();
    }

    fn friend_name(&self, id: u64) -> String {
        self.lock().friend_names.get(&id).cloned().unwrap_or_default()
    }

    fn invite_to_game(&self, friend: u64, connect: &str) -> bool {
        self.lock().calls.push(FakeCall::InviteToGame { friend, connect: connect.to_string() });
        true
    }

    fn launch_command_line(&self) -> String {
        self.lock().launch_command_line.clone()
    }

    fn pump(&self) -> Vec<BackendEvent> {
        std::mem::take(&mut self.lock().queued)
    }
}
