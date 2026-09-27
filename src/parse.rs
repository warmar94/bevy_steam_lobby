//! Pure helpers: connect strings and SteamID64 validation.

/// `"<prefix> <lobby>"` - the connect string used for rich presence and invites.
pub fn connect_string(prefix: &str, lobby: u64) -> String {
    format!("{prefix} {lobby}")
}

/// Find `<prefix> <lobby>` (whitespace separated) anywhere in `text` - a rich-presence connect
/// string, a full command line, or process arguments joined with spaces - and return the lobby
/// id. `<prefix>=<lobby>` is accepted too. `None` when missing, malformed, or `0`.
pub fn parse_connect_lobby(text: &str, prefix: &str) -> Option<u64> {
    let prefix = prefix.trim();
    if prefix.is_empty() {
        return None;
    }
    let mut tokens = text.split_whitespace();
    while let Some(tok) = tokens.next() {
        let tok = tok.trim_matches('"');
        let value = if tok == prefix {
            tokens.next()?
        } else if let Some(rest) = tok.strip_prefix(prefix).and_then(|r| r.strip_prefix('=')) {
            rest
        } else {
            continue;
        };
        return value.trim_matches('"').parse::<u64>().ok().filter(|&id| id != 0);
    }
    None
}

/// Is `id` an individual (user) SteamID64 in the public universe?
/// Universe `id >> 56 == 1`, account type `(id >> 52) & 0xF == 1`, instance
/// `(id >> 32) & 0xFFFFF == 1`, account id `id as u32 != 0`.
pub fn is_individual_steam_id64(id: u64) -> bool {
    let universe = id >> 56;
    let account_type = (id >> 52) & 0xF;
    let instance = (id >> 32) & 0xF_FFFF;
    let account = id as u32;
    universe == 1 && account_type == 1 && instance == 1 && account != 0
}
