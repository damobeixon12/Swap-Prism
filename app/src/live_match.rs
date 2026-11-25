//! Players of the current match from Rocket League's official Stats API (local TCP 49123, read only).
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
pub struct LiveMatch {
    pub playlist: i64,
    /// (name, PrimaryId such as "Epic|id|0", team, score)
    pub players: Vec<(String, String, i64, i64)>,
    /// Bumped when a match ends, so ratings are read again with the result applied.
    pub ended: u64,
}

pub fn apply(state: &mut LiveMatch, message: &Value) {
    let data = match &message["Data"] {
        Value::String(text) => serde_json::from_str(text).unwrap_or(Value::Null),
        other => other.clone(),
    };
    match message["Event"].as_str().unwrap_or("") {
        "UpdateState" => {
            state.playlist = data["Game"]["PlaylistId"].as_i64().unwrap_or(0);
            state.players = data["Players"].as_array().into_iter().flatten().filter_map(|p| {
                Some((p["Name"].as_str()?.to_owned(), p["PrimaryId"].as_str().unwrap_or("").to_owned(), p["TeamNum"].as_i64().unwrap_or(0), p["Score"].as_i64().unwrap_or(0)))
            }).collect();
        }
        "MatchEnded" => state.ended += 1,
        "MatchDestroyed" => state.players.clear(),
        _ => {}
    }
}

/// One background thread; only connects while the game is running (the port refuses otherwise).
pub fn watch(state: Arc<Mutex<LiveMatch>>) {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(move || {
        std::thread::spawn(move || loop {
            if let Ok(stream) = std::net::TcpStream::connect(("127.0.0.1", 49123)) {
                for message in serde_json::Deserializer::from_reader(std::io::BufReader::new(stream)).into_iter::<Value>() {
                    let Ok(message) = message else { break };
                    apply(&mut state.lock().unwrap(), &message);
                }
                state.lock().unwrap().players.clear();
            }
            std::thread::sleep(Duration::from_secs(5));
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_players_and_end() {
        let mut state = LiveMatch::default();
        let update = serde_json::json!({"Event":"UpdateState","Data":r#"{"Players":[{"Name":"a.","PrimaryId":"Epic|x|0","TeamNum":1,"Score":30}],"Game":{"PlaylistId":11}}"#});
        apply(&mut state, &update);
        assert_eq!((state.playlist, state.players.clone()), (11, vec![("a.".into(), "Epic|x|0".into(), 1, 30)]));
        apply(&mut state, &serde_json::json!({"Event":"MatchEnded","Data":"{}"}));
        assert_eq!(state.ended, 1);
        apply(&mut state, &serde_json::json!({"Event":"MatchDestroyed","Data":"{}"}));
        assert!(state.players.is_empty());
    }
}
