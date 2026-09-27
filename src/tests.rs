//! Headless tests: a tiny made-up app (MinimalPlugins + the plugin + the FAKE backend). No real
//! Steam call is ever made here.

use bevy::ecs::schedule::{LogLevel, ScheduleBuildSettings, ScheduleLabel};
use bevy::prelude::*;

use crate::*;

/// A fabricated individual SteamID64 (the placeholder the fake backend and the README use too).
const FRIEND: u64 = 76_561_197_960_265_730;

#[derive(Resource)]
struct Seen<T: Message + Clone>(Vec<T>);

fn collect<T: Message + Clone>(mut r: MessageReader<T>, mut seen: ResMut<Seen<T>>) {
    seen.0.extend(r.read().cloned());
}

fn watch<T: Message + Clone>(app: &mut App) {
    app.insert_resource(Seen::<T>(Vec::new())).add_systems(Last, collect::<T>.after(SteamLobbySystems::Requests));
}

fn seen<T: Message + Clone>(app: &App) -> Vec<T> {
    app.world().resource::<Seen<T>>().0.clone()
}

fn app_with(backend: Option<&FakeSteamBackend>) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(SteamLobbyPlugin::default());
    if let Some(b) = backend {
        app.insert_resource(SteamLobbyBackendRes(Box::new(b.clone())));
    }
    watch::<LobbyCreated>(&mut app);
    watch::<LobbyEntered>(&mut app);
    watch::<JoinRequested>(&mut app);
    watch::<LobbyLeft>(&mut app);
    watch::<InviteSent>(&mut app);
    watch::<LobbyError>(&mut app);
    app
}

fn frames(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn create_msg() -> CreateLobby {
    CreateLobby { kind: LobbyKind::FriendsOnly, max_members: 9, data: vec![("host".into(), "123".into()), ("version".into(), "7".into())] }
}

fn creates(fake: &FakeSteamBackend) -> usize {
    fake.calls().iter().filter(|c| matches!(c, FakeCall::CreateLobby { .. })).count()
}

fn lobby(app: &App) -> SteamLobby {
    app.world().resource::<SteamLobby>().clone()
}

#[test]
fn create_sets_data_joinable_and_connect_presence() {
    let fake = FakeSteamBackend::new();
    let mut app = app_with(Some(&fake));
    app.world_mut().write_message(create_msg());
    frames(&mut app, 3);

    assert_eq!(seen::<LobbyCreated>(&app), vec![LobbyCreated { lobby: 1000 }]);
    assert_eq!(lobby(&app).current, Some(1000));
    assert!(!lobby(&app).pending_create);
    assert_eq!(fake.lobby_data(1000, "host").as_deref(), Some("123"));
    assert_eq!(fake.lobby_data(1000, "version").as_deref(), Some("7"));
    assert!(fake.calls().contains(&FakeCall::CreateLobby { kind: LobbyKind::FriendsOnly, max_members: 9 }));
    assert!(fake.calls().contains(&FakeCall::SetLobbyJoinable { lobby: 1000, joinable: true }));
    assert_eq!(fake.rich_presence("connect").as_deref(), Some("+connect_lobby 1000"));
    assert!(seen::<LobbyError>(&app).is_empty());
}

#[test]
fn a_duplicate_create_is_refused_and_steam_is_called_once() {
    let fake = FakeSteamBackend::new();
    fake.set_auto_complete_create(false);
    let mut app = app_with(Some(&fake));
    app.world_mut().write_message(create_msg());
    app.world_mut().write_message(create_msg());
    frames(&mut app, 2);
    // And again while the first is still pending, in a later frame.
    app.world_mut().write_message(create_msg());
    frames(&mut app, 2);

    assert_eq!(creates(&fake), 1);
    let errs = seen::<LobbyError>(&app);
    assert_eq!(errs.len(), 2);
    assert!(errs.iter().all(|e| e.kind == LobbyErrorKind::AlreadyInLobby));

    // Once created, a further create is still refused.
    fake.complete_create(555);
    frames(&mut app, 1);
    app.world_mut().write_message(create_msg());
    frames(&mut app, 2);
    assert_eq!(creates(&fake), 1);
    assert_eq!(seen::<LobbyError>(&app).len(), 3);
    assert_eq!(lobby(&app).current, Some(555));
}

#[test]
fn leave_clears_everything_and_a_recreate_makes_exactly_one_lobby() {
    let fake = FakeSteamBackend::new();
    let mut app = app_with(Some(&fake));
    app.world_mut().write_message(create_msg());
    frames(&mut app, 3);
    assert_eq!(lobby(&app).current, Some(1000));

    app.world_mut().write_message(LeaveLobby);
    frames(&mut app, 2);
    assert!(fake.calls().contains(&FakeCall::LeaveLobby(1000)));
    assert!(fake.calls().contains(&FakeCall::ClearRichPresence));
    assert_eq!(lobby(&app).current, None);
    assert_eq!(seen::<LobbyLeft>(&app), vec![LobbyLeft { lobby: 1000 }]);
    assert_eq!(fake.rich_presence("connect"), None);

    app.world_mut().write_message(create_msg());
    frames(&mut app, 3);
    assert_eq!(creates(&fake), 2);
    assert_eq!(seen::<LobbyCreated>(&app), vec![LobbyCreated { lobby: 1000 }, LobbyCreated { lobby: 1001 }]);
    assert_eq!(lobby(&app).current, Some(1001));
}

#[test]
fn a_create_completing_after_leave_is_left_and_never_current() {
    let fake = FakeSteamBackend::new();
    fake.set_auto_complete_create(false);
    let mut app = app_with(Some(&fake));
    app.world_mut().write_message(create_msg());
    frames(&mut app, 2);
    assert!(lobby(&app).pending_create);

    app.world_mut().write_message(LeaveLobby);
    frames(&mut app, 2);
    assert!(!lobby(&app).pending_create);

    fake.complete_create(777);
    frames(&mut app, 2);
    assert!(fake.calls().contains(&FakeCall::LeaveLobby(777)));
    assert_eq!(lobby(&app).current, None);
    assert!(seen::<LobbyCreated>(&app).is_empty());
    // No data was ever written to the abandoned lobby.
    assert_eq!(fake.lobby_data(777, "host"), None);

    // Re-host after that: exactly one new lobby, and it becomes current.
    fake.set_auto_complete_create(true);
    app.world_mut().write_message(create_msg());
    frames(&mut app, 3);
    assert_eq!(creates(&fake), 2);
    assert_eq!(lobby(&app).current, Some(1000));
}

#[test]
fn backend_join_requests_become_messages_with_their_source() {
    let fake = FakeSteamBackend::new();
    let mut app = app_with(Some(&fake));
    fake.push_event(BackendEvent::LobbyJoinRequested { lobby: 42, from: FRIEND });
    fake.push_event(BackendEvent::RichPresenceJoinRequested { from: FRIEND, connect: "+connect_lobby 42".into() });
    fake.push_event(BackendEvent::RichPresenceJoinRequested { from: FRIEND, connect: "+connect_lobby banana".into() });
    fake.push_event(BackendEvent::RichPresenceJoinRequested { from: FRIEND, connect: "garbage".into() });
    frames(&mut app, 2);

    assert_eq!(
        seen::<JoinRequested>(&app),
        vec![
            JoinRequested { lobby: 42, from: FRIEND, source: JoinSource::LobbyInvite },
            JoinRequested { lobby: 42, from: FRIEND, source: JoinSource::RichPresence },
        ]
    );
    // The plugin never joins by itself.
    assert!(!fake.calls().iter().any(|c| matches!(c, FakeCall::JoinLobby(_))));
}

#[test]
fn launch_args_are_reported_once() {
    let fake = FakeSteamBackend::new();
    fake.set_launch_command_line("-foo +connect_lobby 99 -bar");
    let mut app = app_with(Some(&fake));
    frames(&mut app, 4);
    assert_eq!(seen::<JoinRequested>(&app), vec![JoinRequested { lobby: 99, from: 0, source: JoinSource::LaunchArgs }]);
}

#[test]
fn join_lobby_success_and_failure() {
    let fake = FakeSteamBackend::new();
    let mut app = app_with(Some(&fake));
    app.world_mut().write_message(JoinLobby { lobby: 42 });
    frames(&mut app, 3);
    assert_eq!(seen::<LobbyEntered>(&app), vec![LobbyEntered { lobby: 42 }]);
    assert_eq!(lobby(&app).current, Some(42));

    // Joining another lobby leaves the old one first.
    fake.set_join_succeeds(false);
    app.world_mut().write_message(JoinLobby { lobby: 43 });
    frames(&mut app, 3);
    assert!(fake.calls().contains(&FakeCall::LeaveLobby(42)));
    assert_eq!(seen::<LobbyLeft>(&app), vec![LobbyLeft { lobby: 42 }]);
    let errs = seen::<LobbyError>(&app);
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].kind, LobbyErrorKind::JoinFailed);
    assert_eq!(lobby(&app).current, None);
    assert_eq!(lobby(&app).pending_join, None);
}

#[test]
fn invite_needs_a_lobby_and_uses_the_connect_string() {
    let fake = FakeSteamBackend::new();
    let mut app = app_with(Some(&fake));
    app.world_mut().write_message(InviteFriend { steam_id: FRIEND });
    frames(&mut app, 2);
    let errs = seen::<LobbyError>(&app);
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].kind, LobbyErrorKind::NoLobby);
    assert!(!fake.calls().iter().any(|c| matches!(c, FakeCall::InviteToGame { .. })));

    // A bad id is refused without a call either.
    app.world_mut().write_message(InviteFriend { steam_id: 12345 });
    frames(&mut app, 1);
    assert_eq!(seen::<LobbyError>(&app).last().map(|e| e.kind), Some(LobbyErrorKind::InvalidSteamId));

    app.world_mut().write_message(create_msg());
    frames(&mut app, 3);
    app.world_mut().write_message(InviteFriend { steam_id: FRIEND });
    frames(&mut app, 2);
    assert!(fake.calls().contains(&FakeCall::InviteToGame { friend: FRIEND, connect: "+connect_lobby 1000".into() }));
    assert_eq!(seen::<InviteSent>(&app), vec![InviteSent { steam_id: FRIEND, lobby: 1000, ok: true }]);
}

#[test]
fn without_a_backend_requests_get_no_backend_errors_and_nothing_panics() {
    let mut app = app_with(None);
    app.world_mut().write_message(create_msg());
    app.world_mut().write_message(LeaveLobby);
    app.world_mut().write_message(ClearRichPresence);
    frames(&mut app, 3);
    let errs = seen::<LobbyError>(&app);
    assert_eq!(errs.len(), 1, "leave/clear are silent no-ops without Steam: {errs:?}");
    assert_eq!(errs[0].kind, LobbyErrorKind::NoBackend);
    assert_eq!(lobby(&app), SteamLobby::default());
}

#[test]
fn app_exit_leaves_the_lobby() {
    let fake = FakeSteamBackend::new();
    let mut app = app_with(Some(&fake));
    app.world_mut().write_message(create_msg());
    frames(&mut app, 3);
    app.world_mut().write_message(AppExit::Success);
    frames(&mut app, 1);
    assert!(fake.calls().contains(&FakeCall::LeaveLobby(1000)));
    assert!(fake.calls().contains(&FakeCall::ClearRichPresence));
    assert_eq!(lobby(&app).current, None);
}

#[test]
fn no_ambiguous_systems_in_first_update_or_last() {
    let fake = FakeSteamBackend::new();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(SteamLobbyPlugin::default());
    app.insert_resource(SteamLobbyBackendRes(Box::new(fake.clone())));
    for label in [First.intern(), Update.intern(), Last.intern()] {
        app.edit_schedule(label, |s| {
            s.set_build_settings(ScheduleBuildSettings { ambiguity_detection: LogLevel::Error, ..default() });
        });
    }
    app.world_mut().write_message(create_msg());
    frames(&mut app, 3);
    assert_eq!(app.world().resource::<SteamLobby>().current, Some(1000));
}

#[test]
fn parse_connect_lobby_cases() {
    let p = "+connect_lobby";
    assert_eq!(parse_connect_lobby("+connect_lobby 123", p), Some(123));
    assert_eq!(parse_connect_lobby("game.exe -x +connect_lobby 109775241000000000 -y", p), Some(109_775_241_000_000_000));
    assert_eq!(parse_connect_lobby("+connect_lobby=77", p), Some(77));
    assert_eq!(parse_connect_lobby("  +connect_lobby   5 ", p), Some(5));
    assert_eq!(parse_connect_lobby("+connect_lobby", p), None);
    assert_eq!(parse_connect_lobby("+connect_lobby abc", p), None);
    assert_eq!(parse_connect_lobby("+connect_lobby 0", p), None);
    assert_eq!(parse_connect_lobby("+connect_lobby -5", p), None);
    assert_eq!(parse_connect_lobby("+connect_lobbyx 5", p), None);
    assert_eq!(parse_connect_lobby("", p), None);
    assert_eq!(parse_connect_lobby("+connect_lobby 5", ""), None);
    assert_eq!(parse_connect_lobby("+join 8", "+join"), Some(8));
    let args = ["game.exe".to_string(), "+connect_lobby".to_string(), "31".to_string()];
    assert_eq!(parse_connect_lobby(&args.join(" "), p), Some(31));
    assert_eq!(connect_string(p, 9), "+connect_lobby 9");
}

#[test]
fn steam_id64_validation() {
    assert!(is_individual_steam_id64(76_561_198_000_000_042)); // fabricated
    assert!(is_individual_steam_id64(76_561_197_960_265_729));
    assert!(!is_individual_steam_id64(0));
    assert!(!is_individual_steam_id64(1));
    assert!(!is_individual_steam_id64(12345));
    // Account id 0 in an otherwise valid individual prefix.
    assert!(!is_individual_steam_id64(76_561_197_960_265_728));
    // A clan/group id (account type 7, instance 0).
    assert!(!is_individual_steam_id64(103_582_791_429_521_408));
    // A lobby / chat id (account type 8).
    assert!(!is_individual_steam_id64(109_775_241_000_000_000));
    // Wrong universe (2 = internal).
    assert!(!is_individual_steam_id64((2u64 << 56) | (1 << 52) | (1 << 32) | 5));
}
