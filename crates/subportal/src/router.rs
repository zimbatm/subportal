use subportal_iroh::control::FocusState;
use subportal_lib::protocol::Request;

/// Metadata about a connected client, used for routing decisions.
pub struct ClientInfo {
    pub endpoint_id: String,
    pub focus: FocusState,
    pub capabilities: Vec<String>,
}

/// Routing strategy for a request.
pub enum Strategy {
    /// Pick the single best client with the given capability.
    PickBest(&'static str),
    /// Fan out to all clients with the given capability.
    FanOut(&'static str),
    /// The agent responds directly (no client needed).
    Direct,
}

/// Determine the routing strategy for a request.
pub fn strategy_for(request: &Request) -> Strategy {
    match request {
        Request::OpenURI { .. } => Strategy::PickBest("OpenURI"),
        Request::OpenFile { .. } => Strategy::PickBest("OpenFile"),
        Request::Notify { .. } => Strategy::FanOut("Notify"),
        _ => Strategy::Direct,
    }
}

/// Pick the best client for a given capability.
///
/// Prefers Active over Idle focus state.
pub fn pick_best<'a>(clients: &'a [ClientInfo], capability: &str) -> Option<&'a ClientInfo> {
    let mut candidates: Vec<_> = clients
        .iter()
        .filter(|c| c.capabilities.iter().any(|cap| cap == capability))
        .collect();

    if candidates.is_empty() {
        return None;
    }

    // Sort: Active first, then Idle
    candidates.sort_by_key(|c| match c.focus {
        FocusState::Active => 0,
        FocusState::Idle => 1,
    });

    Some(candidates[0])
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

    fn make_client(id: &str, focus: FocusState, caps: &[&str]) -> ClientInfo {
        ClientInfo {
            endpoint_id: id.into(),
            focus,
            capabilities: caps.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn pick_best_prefers_active() {
        let clients = vec![
            make_client("idle", FocusState::Idle, &["OpenURI"]),
            make_client("active", FocusState::Active, &["OpenURI"]),
        ];
        let best = pick_best(&clients, "OpenURI").unwrap();
        assert_eq!(best.endpoint_id, "active");
    }

    #[test]
    fn pick_best_filters_capability() {
        let clients = vec![
            make_client("no-cap", FocusState::Active, &["Notify"]),
            make_client("has-cap", FocusState::Idle, &["OpenURI"]),
        ];
        let best = pick_best(&clients, "OpenURI").unwrap();
        assert_eq!(best.endpoint_id, "has-cap");
    }

    #[test]
    fn pick_best_none_when_empty() {
        let clients: Vec<ClientInfo> = vec![];
        assert!(pick_best(&clients, "OpenURI").is_none());
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
            strategy_for(&Request::OpenURI {
                uri: "x".into()
            }),
            Strategy::PickBest("OpenURI")
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
        assert!(matches!(
            strategy_for(&Request::Ping {}),
            Strategy::Direct
        ));
    }
}
