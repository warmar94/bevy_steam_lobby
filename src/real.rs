//! [`RealSteamBackend`] (feature `steam`): the backend over `steamworks` 0.12.2.
//!
//! Verified against the locked `steamworks-0.12.2` source:
//! - `Client::process_callbacks` (lib.rs:319) runs `Inner::run_callbacks_raw` (lib.rs:154), which
//!   ALSO dispatches call results (the `create_lobby` / `join_lobby` closures, lib.rs:161-181).
//!   So `process_callbacks` REPLACES `run_callbacks` (lib.rs:305): call exactly one per frame. We
//!   call `process_callbacks` from [`SteamLobbyBackend::pump`] and nothing else may call either.
//! - `GameLobbyJoinRequested` / `GameRichPresenceJoinRequested` are mapped by
//!   `CallbackResult::from_raw` (callback.rs:67, :76) - no `register_callback` handle needed.
//!   `LobbyChatUpdate` is deliberately never touched (its conversion `unreachable!()`s on unknown
//!   states in 0.12.2).
//! - `Matchmaking` / `Friends` hold raw pointers (not `Send`): fetched fresh from the `Client`
//!   every call, never stored.
//! - `create_lobby` `assert!(max_members <= 250)` (matchmaking.rs:90) -> we clamp to `1..=250`.
//! - `lobby_data` / `set_lobby_data` / `set_rich_presence` / `invite_user_to_game` build
//!   `CString::new(..).unwrap()` -> a string with an interior NUL would PANIC inside steamworks;
//!   every such input is rejected here first.
//! - `invite_user_to_game` returns `()` (friends.rs:470): our `bool` is "the call was made".

use std::sync::{Arc, Mutex};

use steamworks::{CallbackResult, Client, LobbyId, LobbyType, SteamId};

use crate::backend::{BackendEvent, LobbyKind, SteamLobbyBackend};

type Queue = Arc<Mutex<Vec<BackendEvent>>>;

fn push(queue: &Queue, ev: BackendEvent) {
    queue.lock().unwrap_or_else(|p| p.into_inner()).push(ev);
}

fn has_nul(s: &str) -> bool {
    s.contains('\0')
}

/// The real Steam backend. Holds a `steamworks::Client` clone (which also keeps Steam alive).
pub struct RealSteamBackend {
    client: Client,
    queue: Queue,
}

impl RealSteamBackend {
    /// Wrap an initialised Steam client. The caller keeps its own clone for other uses.
    pub fn new(client: Client) -> Self {
        Self { client, queue: Arc::new(Mutex::new(Vec::new())) }
    }
}

impl SteamLobbyBackend for RealSteamBackend {
    fn local_id(&self) -> u64 {
        self.client.user().steam_id().raw()
    }

    fn create_lobby(&self, kind: LobbyKind, max_members: u32) {
        let ty = match kind {
            LobbyKind::Private => LobbyType::Private,
            LobbyKind::FriendsOnly => LobbyType::FriendsOnly,
            LobbyKind::Public => LobbyType::Public,
            LobbyKind::Invisible => LobbyType::Invisible,
        };
        let queue = self.queue.clone();
        self.client.matchmaking().create_lobby(ty, max_members.clamp(1, 250), move |res| {
            let ev = match res {
                Ok(id) => BackendEvent::LobbyCreated { lobby: id.raw() },
                Err(e) => BackendEvent::LobbyCreateFailed { message: e.to_string() },
            };
            push(&queue, ev);
        });
    }

    fn join_lobby(&self, lobby: u64) {
        let queue = self.queue.clone();
        self.client.matchmaking().join_lobby(LobbyId::from_raw(lobby), move |res| {
            let ev = match res {
                Ok(id) => BackendEvent::LobbyEntered { lobby: id.raw() },
                Err(()) => BackendEvent::LobbyJoinFailed { lobby },
            };
            push(&queue, ev);
        });
    }

    fn leave_lobby(&self, lobby: u64) {
        self.client.matchmaking().leave_lobby(LobbyId::from_raw(lobby));
    }

    fn set_lobby_data(&self, lobby: u64, key: &str, value: &str) -> bool {
        if has_nul(key) || has_nul(value) {
            return false;
        }
        self.client.matchmaking().set_lobby_data(LobbyId::from_raw(lobby), key, value)
    }

    fn lobby_data(&self, lobby: u64, key: &str) -> Option<String> {
        if has_nul(key) {
            return None;
        }
        self.client.matchmaking().lobby_data(LobbyId::from_raw(lobby), key).map(|s| s.to_string())
    }

    fn lobby_member_count(&self, lobby: u64) -> usize {
        self.client.matchmaking().lobby_member_count(LobbyId::from_raw(lobby))
    }

    fn set_lobby_joinable(&self, lobby: u64, joinable: bool) -> bool {
        self.client.matchmaking().set_lobby_joinable(LobbyId::from_raw(lobby), joinable)
    }

    fn set_rich_presence(&self, key: &str, value: Option<&str>) -> bool {
        if has_nul(key) || value.is_some_and(has_nul) {
            return false;
        }
        self.client.friends().set_rich_presence(key, value)
    }

    fn clear_rich_presence(&self) {
        self.client.friends().clear_rich_presence();
    }

    fn friend_name(&self, id: u64) -> String {
        self.client.friends().get_friend(SteamId::from_raw(id)).name()
    }

    fn invite_to_game(&self, friend: u64, connect: &str) -> bool {
        if has_nul(connect) {
            return false;
        }
        self.client.friends().get_friend(SteamId::from_raw(friend)).invite_user_to_game(connect);
        true
    }

    fn launch_command_line(&self) -> String {
        self.client.apps().launch_command_line()
    }

    fn pump(&self) -> Vec<BackendEvent> {
        // Callbacks first; the create/join closures run INSIDE this call and push to `queue`, so
        // the queue must not be locked while it runs.
        let mut out = Vec::new();
        self.client.process_callbacks(|cb| match cb {
            CallbackResult::GameLobbyJoinRequested(r) => {
                out.push(BackendEvent::LobbyJoinRequested { lobby: r.lobby_steam_id.raw(), from: r.friend_steam_id.raw() })
            }
            CallbackResult::GameRichPresenceJoinRequested(r) => {
                out.push(BackendEvent::RichPresenceJoinRequested { from: r.friend_steam_id.raw(), connect: r.connect })
            }
            _ => {}
        });
        let mut q = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        out.append(&mut q);
        out
    }
}
