use std::time::Instant;

use subportal_iroh::control::FocusState;
use subportal_lib::protocol::Request;

/// Metadata about a connected client, used for routing decisions.
pub struct ClientInfo {
    pub endpoint_id: String,
    pub focus: FocusState,
    pub capabilities: Vec<String>,
    // Carried through for the upcoming per-device preference / notification DND
    // policy; not read by the current strategies yet.
    #[allow(dead_code)]
    pub platform: String,
    /// When this client was last known to be in active use (connect, focus →
    /// active, or user interaction). Drives recency-based routing.
    pub last_active: Instant,
}

/// Routing strategy for a request — the per-capability policy.
pub enum Strategy {
    /// Single target: try the ranked clients in order, failing over to the next
    /// on a transport failure. For opening URIs/files — it lands on exactly one
    /// device, the best reachable one.
    Failover(&'static str),
    /// Race all capable clients concurrently; the first user *decision* wins
    /// (a transport failure doesn't count). For Confirm — approve from whichever
    /// device you're actually at.
    Race(&'static str),
    /// Fan out to all clients with the given capability. For notifications.
    FanOut(&'static str),
    /// The agent responds directly (no client needed).
    Direct,
}

/// Determine the routing strategy for a request.
pub fn strategy_for(request: &Request) -> Strategy {
    match request {
        Request::OpenURI { .. } => Strategy::Failover("OpenURI"),
        Request::OpenFile { .. } => Strategy::Failover("OpenFile"),
        Request::Confirm { .. } => Strategy::Race("Confirm"),
        Request::Notify { .. } => Strategy::FanOut("Notify"),
        _ => Strategy::Direct,
    }
}

/// Rank the clients with a given capability, best first.
///
/// Total, deterministic order: active before idle, then most-recently-active,
/// then endpoint id as a stable tiebreak. This is also the failover order for
/// single-target requests — if the head is unreachable, try the next.
pub fn rank<'a>(clients: &'a [ClientInfo], capability: &str) -> Vec<&'a ClientInfo> {
    let mut candidates: Vec<&ClientInfo> = clients
        .iter()
        .filter(|c| c.capabilities.iter().any(|cap| cap == capability))
        .collect();

    candidates.sort_by(|a, b| {
        focus_rank(a.focus)
            .cmp(&focus_rank(b.focus))
            .then(b.last_active.cmp(&a.last_active)) // more recent first
            .then(a.endpoint_id.cmp(&b.endpoint_id)) // stable tiebreak
    });

    candidates
}

fn focus_rank(focus: FocusState) -> u8 {
    match focus {
        FocusState::Active => 0,
        FocusState::Idle => 1,
    }
}

/// Return all clients with a given capability.
pub fn fan_out<'a>(clients: &'a [ClientInfo], capability: &str) -> Vec<&'a ClientInfo> {
    clients
        .iter()
        .filter(|c| c.capabilities.iter().any(|cap| cap == capability))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Head of the ranked list — the single best client, used by the tests.
    fn best<'a>(clients: &'a [ClientInfo], cap: &str) -> Option<&'a ClientInfo> {
        rank(clients, cap).into_iter().next()
    }

    fn make_client(id: &str, focus: FocusState, caps: &[&str]) -> ClientInfo {
        make_client_at(id, focus, caps, Instant::now())
    }

    fn make_client_at(
        id: &str,
        focus: FocusState,
        caps: &[&str],
        last_active: Instant,
    ) -> ClientInfo {
        ClientInfo {
            endpoint_id: id.into(),
            focus,
            capabilities: caps.iter().map(|s| (*s).to_string()).collect(),
            platform: "linux".into(),
            last_active,
        }
    }

    #[test]
    fn pick_best_prefers_active() {
        let clients = vec![
            make_client("idle", FocusState::Idle, &["OpenURI"]),
            make_client("active", FocusState::Active, &["OpenURI"]),
        ];
        let best = best(&clients, "OpenURI").unwrap();
        assert_eq!(best.endpoint_id, "active");
    }

    #[test]
    fn pick_best_filters_capability() {
        let clients = vec![
            make_client("no-cap", FocusState::Active, &["Notify"]),
            make_client("has-cap", FocusState::Idle, &["OpenURI"]),
        ];
        let best = best(&clients, "OpenURI").unwrap();
        assert_eq!(best.endpoint_id, "has-cap");
    }

    #[test]
    fn pick_best_none_when_empty() {
        let clients: Vec<ClientInfo> = vec![];
        assert!(best(&clients, "OpenURI").is_none());
    }

    #[test]
    fn among_equal_focus_prefers_most_recently_active() {
        let older = Instant::now();
        let newer = older + Duration::from_secs(5);
        // Registry/HashMap order should not matter: the recent one wins either way.
        let clients = vec![
            make_client_at("stale", FocusState::Active, &["OpenURI"], older),
            make_client_at("recent", FocusState::Active, &["OpenURI"], newer),
        ];
        assert_eq!(best(&clients, "OpenURI").unwrap().endpoint_id, "recent");

        let clients_rev = vec![
            make_client_at("recent", FocusState::Active, &["OpenURI"], newer),
            make_client_at("stale", FocusState::Active, &["OpenURI"], older),
        ];
        assert_eq!(best(&clients_rev, "OpenURI").unwrap().endpoint_id, "recent");
    }

    #[test]
    fn rank_is_the_failover_order() {
        let t = Instant::now();
        let clients = vec![
            make_client_at(
                "idle-recent",
                FocusState::Idle,
                &["OpenURI"],
                t + Duration::from_secs(9),
            ),
            make_client_at("active-stale", FocusState::Active, &["OpenURI"], t),
            make_client_at(
                "active-recent",
                FocusState::Active,
                &["OpenURI"],
                t + Duration::from_secs(5),
            ),
        ];
        let order: Vec<_> = rank(&clients, "OpenURI")
            .into_iter()
            .map(|c| c.endpoint_id.as_str())
            .collect();
        // active before idle; within active, most-recent first.
        assert_eq!(order, ["active-recent", "active-stale", "idle-recent"]);
    }

    #[test]
    fn fan_out_returns_all_capable() {
        let clients = vec![
            make_client("a", FocusState::Active, &["Notify"]),
            make_client("b", FocusState::Idle, &["Notify"]),
            make_client("c", FocusState::Active, &["OpenURI"]),
        ];
        let result = fan_out(&clients, "Notify");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn strategy_for_requests() {
        assert!(matches!(
            strategy_for(&Request::OpenURI { uri: "x".into() }),
            Strategy::Failover("OpenURI")
        ));
        assert!(matches!(
            strategy_for(&Request::Notify {
                title: "x".into(),
                body: None,
                urgency: None,
                icon: None,
            }),
            Strategy::FanOut("Notify")
        ));
        assert!(matches!(strategy_for(&Request::Ping {}), Strategy::Direct));
        assert!(matches!(
            strategy_for(&Request::Confirm {
                message: "ok?".into(),
                title: None,
            }),
            Strategy::Race("Confirm")
        ));
    }
}
