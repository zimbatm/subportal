# Multi-client routing

When more than one client is enrolled with an agent (say a desktop and a
phone), the agent has to decide *where* each request goes. This note explains
the model on this branch and what is deliberately left for later.

## The problem with the first cut

The original router keyed only on a binary focus state (`Active | Idle`) and,
on a tie, took whatever client the `HashMap` happened to yield first. In
practice that meant:

- **Nondeterministic target.** Two `Active` clients → an `xdg-open` URL opened
  on a *random* device.
- **Weak signal.** Focus is polled from the screensaver every 30 s and defaults
  to `Active` on any error — and a phone can't report it at all, so it is
  permanently `Active`. The tie is the common case, so the randomness dominated.
- **No device class.** `platform` was received at hello and thrown away, so the
  router couldn't prefer "desktop for URLs."
- **Notify blasted every device; Confirm went to one arbitrary device** with no
  failover — if it was unattended-but-`Active`, the approval just hung.

## The model on this branch

**Presence.** Each connected client carries `platform` and a `last_active`
timestamp, bumped on connect and on every focus → active transition.

**Deterministic ranking** (`router::rank`). Candidates for a capability are put
in a total order: active focus first, then most-recently-active, then endpoint
id as a stable tiebreak. `rank()` returns the whole ordered list; its head is
"the best client" and the list *is* the failover order.

**Per-capability policy** (`router::Strategy`):

| capability | strategy | behaviour |
| --- | --- | --- |
| OpenURI / OpenFile | `Failover` | send to the best reachable device; fail over to the next *only* on a transport failure. A user approve/deny is final, not retried elsewhere. |
| Confirm | `Race` | send to every capable device at once; the first user *decision* wins. Transport failures don't count, so a dead device can't decide. |
| Notify | `FanOut` | all capable devices (unchanged). |
| Ping / tickets / revoke | `Direct` | answered by the agent itself. |

Routing now lives in one place: the agent's per-strategy dispatch. `Hub`'s old
`route_request` (only ever hit for `Direct`) is now `handle_direct`.

## Open decisions

These are the choices baked into the current defaults — each is a knob we may
want to expose or flip:

1. **URL routing: automatic vs sticky.** Today it's automatic by recency. A
   "current device" the user flips explicitly (auto-failing-over when it goes
   away) would be more predictable. Undecided.
2. **Confirm: race-all vs single-best-with-failover.** Currently race-all — the
   point being you can approve from wherever you are. Cost: every device buzzes.
3. **Notify: fan-out vs active-only.** Currently fan-out. Active-only would kill
   the desktop+phone double-buzz, but the focus signal is too unreliable to risk
   dropping notifications on.
4. **Where device priority/preference lives.** Nowhere yet — `platform` is
   carried but unread. Candidates: enrollment metadata, an agent-side config, or
   a per-request `--device` override.

## Follow-up work

- **Cancel-losers control message.** After a `Race` is decided, the losing
  Confirm dialogs linger until their client-side timeout. Add a `Cancel { id }`
  control message so the agent can dismiss them immediately. This is the
  roughest edge of the race today.
- **Notify DND / active-only** using `platform` + focus (decision 3).
- **Per-device preference / priority** (decision 4): e.g. "URLs → desktop,
  notifications → phone," plus a per-request override.
- **Session affinity.** Remember which device the last interactive request for a
  given origin went to, and prefer it — so a `notify → open-url` pair lands on
  the same screen.
- **Richer presence.** A three-state model (active / idle / away-or-locked) and
  bumping `last_active` on real user interaction (a click/approve is the
  strongest "I'm here" signal), not just the 30 s screensaver poll.
- **Race scope.** Decide whether `Race` should hit *all* capable devices or only
  attended ones once presence is trustworthy enough to tell them apart.
