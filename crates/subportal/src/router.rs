use std::future::Future;
use std::time::Instant;

use subportal_iroh::control::FocusState;
use subportal_lib::protocol::{Request, Response, SubportalError};
use tracing::{info, warn};

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

/// Resolve a Failover dispatch: try targets in ranked order via `send`.
/// A transport failure (`NoClient`) moves on to the next target; any other
/// outcome — success or a user decision like `UserDenied` — is final.
pub async fn failover_decision<T, F, Fut>(
    targets: Vec<(String, T)>,
    mut send: F,
) -> Result<Response, SubportalError>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Result<Response, SubportalError>>,
{
    for (eid, target) in targets {
        match send(target).await {
            Ok(resp) => return Ok(resp),
            Err(SubportalError::NoClient) => {
                warn!(endpoint_id = %eid, "client unreachable, failing over");
            }
            Err(e) => return Err(e),
        }
    }
    Err(SubportalError::NoClient)
}

/// Resolve a Race dispatch: the first user *decision* wins — an approval
/// (`Ok`) or a denial. `NoClient` (unreachable) and `NoDecision` (prompt
/// expired unanswered) don't get a vote: a device timing out at 60s must not
/// veto the user approving at 61s on another device. When nobody decides:
/// `NoDecision` if at least one device was asked, else `NoClient`.
pub async fn race_decision(
    mut set: tokio::task::JoinSet<(String, Result<Response, SubportalError>)>,
) -> Result<Response, SubportalError> {
    let mut asked_without_answer = false;
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok((eid, Ok(resp))) => {
                info!(endpoint_id = %eid, "confirm approved");
                return Ok(resp);
            }
            Ok((_eid, Err(SubportalError::NoClient))) => {}
            Ok((_eid, Err(SubportalError::NoDecision))) => {
                asked_without_answer = true;
            }
            // A real decision (e.g. UserDenied) from any device wins.
            Ok((eid, Err(e))) => {
                info!(endpoint_id = %eid, "confirm decided: {e}");
                return Err(e);
            }
            Err(_join_err) => {}
        }
    }
    if asked_without_answer {
        Err(SubportalError::NoDecision)
    } else {
        Err(SubportalError::NoClient)
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

    // -- failover_decision ---------------------------------------------------

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    type Outcome = Result<Response, SubportalError>;

    fn target(id: &str, outcome: Outcome) -> (String, Outcome) {
        (id.into(), outcome)
    }

    /// Wrap outcomes in a send fn that counts how many targets were tried.
    fn counting_send(
        calls: &Arc<AtomicUsize>,
    ) -> impl FnMut(Outcome) -> std::future::Ready<Outcome> {
        let calls = calls.clone();
        move |o| {
            calls.fetch_add(1, Ordering::SeqCst);
            std::future::ready(o)
        }
    }

    #[tokio::test]
    async fn failover_empty_is_no_client() {
        let calls = Arc::new(AtomicUsize::new(0));
        let r = failover_decision(vec![], counting_send(&calls)).await;
        assert_eq!(r, Err(SubportalError::NoClient));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn failover_skips_unreachable_client() {
        let calls = Arc::new(AtomicUsize::new(0));
        let targets = vec![
            target("dead", Err(SubportalError::NoClient)),
            target("alive", Ok(Response::Ok)),
        ];
        let r = failover_decision(targets, counting_send(&calls)).await;
        assert_eq!(r, Ok(Response::Ok));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn failover_first_success_stops() {
        let calls = Arc::new(AtomicUsize::new(0));
        let targets = vec![
            target("a", Ok(Response::Ok)),
            target("b", Ok(Response::Ok)),
        ];
        let r = failover_decision(targets, counting_send(&calls)).await;
        assert_eq!(r, Ok(Response::Ok));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    /// A denial must not fail over: the user already answered.
    #[tokio::test]
    async fn failover_user_decision_is_final() {
        let calls = Arc::new(AtomicUsize::new(0));
        let targets = vec![
            target("denier", Err(SubportalError::UserDenied)),
            target("next", Ok(Response::Ok)),
        ];
        let r = failover_decision(targets, counting_send(&calls)).await;
        assert_eq!(r, Err(SubportalError::UserDenied));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn failover_all_unreachable_is_no_client() {
        let calls = Arc::new(AtomicUsize::new(0));
        let targets = vec![
            target("a", Err(SubportalError::NoClient)),
            target("b", Err(SubportalError::NoClient)),
        ];
        let r = failover_decision(targets, counting_send(&calls)).await;
        assert_eq!(r, Err(SubportalError::NoClient));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    // -- race_decision -------------------------------------------------------

    /// Build a JoinSet where each task resolves to `outcome` after `delay_ms`.
    /// Paused-clock tests make the ordering deterministic.
    fn race_set(tasks: Vec<(&str, u64, Outcome)>) -> tokio::task::JoinSet<(String, Outcome)> {
        let mut set = tokio::task::JoinSet::new();
        for (eid, delay_ms, outcome) in tasks {
            let eid = eid.to_string();
            set.spawn(async move {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                (eid, outcome)
            });
        }
        set
    }

    #[tokio::test(start_paused = true)]
    async fn race_empty_is_no_client() {
        let r = race_decision(tokio::task::JoinSet::new()).await;
        assert_eq!(r, Err(SubportalError::NoClient));
    }

    #[tokio::test(start_paused = true)]
    async fn race_approval_wins() {
        let set = race_set(vec![
            ("fast-approve", 10, Ok(Response::Ok)),
            ("slow-deny", 100, Err(SubportalError::UserDenied)),
        ]);
        assert_eq!(race_decision(set).await, Ok(Response::Ok));
    }

    #[tokio::test(start_paused = true)]
    async fn race_denial_wins() {
        let set = race_set(vec![
            ("fast-deny", 10, Err(SubportalError::UserDenied)),
            ("slow-approve", 100, Ok(Response::Ok)),
        ]);
        assert_eq!(race_decision(set).await, Err(SubportalError::UserDenied));
    }

    #[tokio::test(start_paused = true)]
    async fn race_transport_failure_does_not_decide() {
        let set = race_set(vec![
            ("dead", 10, Err(SubportalError::NoClient)),
            ("alive", 100, Ok(Response::Ok)),
        ]);
        assert_eq!(race_decision(set).await, Ok(Response::Ok));
    }

    #[tokio::test(start_paused = true)]
    async fn race_all_unreachable_is_no_client() {
        let set = race_set(vec![
            ("a", 10, Err(SubportalError::NoClient)),
            ("b", 20, Err(SubportalError::NoClient)),
        ]);
        assert_eq!(race_decision(set).await, Err(SubportalError::NoClient));
    }

    /// A prompt expiring unanswered on one device must not veto the user
    /// approving later on another.
    #[tokio::test(start_paused = true)]
    async fn race_unanswered_prompt_does_not_decide() {
        let set = race_set(vec![
            ("timed-out", 10, Err(SubportalError::NoDecision)),
            ("slow-approve", 100, Ok(Response::Ok)),
        ]);
        assert_eq!(race_decision(set).await, Ok(Response::Ok));
    }

    #[tokio::test(start_paused = true)]
    async fn race_all_unanswered_is_no_decision() {
        let set = race_set(vec![
            ("a", 10, Err(SubportalError::NoDecision)),
            ("b", 20, Err(SubportalError::NoDecision)),
        ]);
        assert_eq!(race_decision(set).await, Err(SubportalError::NoDecision));
    }

    #[tokio::test(start_paused = true)]
    async fn race_unanswered_beats_unreachable_in_reporting() {
        let set = race_set(vec![
            ("dead", 10, Err(SubportalError::NoClient)),
            ("asked", 20, Err(SubportalError::NoDecision)),
        ]);
        assert_eq!(race_decision(set).await, Err(SubportalError::NoDecision));
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
