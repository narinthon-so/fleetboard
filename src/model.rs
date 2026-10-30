//! Status machine, derived states and human-only rules. Pure: no I/O.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Draft,
    Todo,
    InProgress,
    Review,
    Done,
    Dropped,
}

impl Status {
    pub const ALL: [Status; 6] = [
        Status::Draft,
        Status::Todo,
        Status::InProgress,
        Status::Review,
        Status::Done,
        Status::Dropped,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Draft => "draft",
            Status::Todo => "todo",
            Status::InProgress => "in_progress",
            Status::Review => "review",
            Status::Done => "done",
            Status::Dropped => "dropped",
        }
    }

    pub fn parse(s: &str) -> Option<Status> {
        Status::ALL.into_iter().find(|v| v.as_str() == s)
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Status::Done | Status::Dropped)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Code,
    Note,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Code => "code",
            Kind::Note => "note",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "code" => Some(Kind::Code),
            "note" => Some(Kind::Note),
            _ => None,
        }
    }
}

/// Everything that moves a task or needs a role check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Promote,
    Claim,
    /// Take over an active claim (human or trusted only).
    Steal,
    Release,
    /// Agent finishes: in_progress -> review.
    Done,
    Verify,
    Reject,
    Drop,
    Reopen,
    /// Plan approval; does not change status.
    Approve,
}

impl Action {
    pub fn name(self) -> &'static str {
        match self {
            Action::Promote => "promote",
            Action::Claim => "claim",
            Action::Steal => "claim --steal",
            Action::Release => "release",
            Action::Done => "done",
            Action::Verify => "verify",
            Action::Reject => "reject",
            Action::Drop => "drop",
            Action::Reopen => "reopen",
            Action::Approve => "approve",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Human,
    /// Agent with the tracker role (D2), set by the human only.
    Trusted,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    #[error("cannot {action} a task that is {from}")]
    BadTransition {
        action: &'static str,
        from: &'static str,
    },
    #[error("{0} is human-only (run it with --as-human from a shell with no agent markers)")]
    HumanOnly(&'static str),
    #[error(
        "plan not approved: write one with `fb plan`, then a human approves it with `fb approve`, before `fb done`"
    )]
    PlanNotApproved,
}

/// Role check. Claim and done are open to everyone; steal needs human or trusted;
/// everything else that governs the board is human-only.
pub fn authorize(action: Action, role: Role) -> Result<(), ModelError> {
    let ok = match action {
        Action::Claim | Action::Done => true,
        Action::Steal => matches!(role, Role::Human | Role::Trusted),
        _ => role == Role::Human,
    };
    if ok {
        Ok(())
    } else {
        Err(ModelError::HumanOnly(action.name()))
    }
}

/// Human-only operations that are not task actions (trust, project alias).
pub fn require_human(role: Role, what: &'static str) -> Result<(), ModelError> {
    if role == Role::Human {
        Ok(())
    } else {
        Err(ModelError::HumanOnly(what))
    }
}

/// Status a freshly added task starts in (D2).
pub fn initial_status(role: Role) -> Status {
    match role {
        Role::Human | Role::Trusted => Status::Todo,
        Role::Agent => Status::Draft,
    }
}

/// Next stored status. Readiness (blockers) and claim ownership are checked by the caller.
pub fn transition(from: Status, action: Action) -> Result<Status, ModelError> {
    use Status as S;
    let to = match (action, from) {
        (Action::Promote, S::Draft) => S::Todo,
        (Action::Claim, S::Todo) => S::InProgress,
        (Action::Steal, S::InProgress) => S::InProgress,
        (Action::Release, S::InProgress) => S::Todo,
        (Action::Done, S::InProgress) => S::Review,
        (Action::Verify, S::Review) => S::Done,
        (Action::Reject, S::Review) => S::InProgress,
        (Action::Drop, s) if !s.is_terminal() => S::Dropped,
        (Action::Reopen, S::Done | S::Dropped) => S::Todo,
        (Action::Approve, S::InProgress) => S::InProgress,
        _ => {
            return Err(ModelError::BadTransition {
                action: action.name(),
                from: from.as_str(),
            });
        }
    };
    Ok(to)
}

/// `fb done` is refused while the plan gate is set and not yet approved.
pub fn check_done_gate(needs_plan_approval: bool, plan_approved: bool) -> Result<(), ModelError> {
    if needs_plan_approval && !plan_approved {
        Err(ModelError::PlanNotApproved)
    } else {
        Ok(())
    }
}

/// Inputs for the derived states (D6).
#[derive(Debug, Clone)]
pub struct Facts<'a> {
    pub status: Status,
    pub kind: Kind,
    pub needs_plan_approval: bool,
    pub has_plan: bool,
    pub plan_approved: bool,
    pub blockers: &'a [Status],
    pub claim_stale: bool,
    /// Commits in the evidence, when known. `None` means not computed.
    pub commits: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Derived {
    pub ready: bool,
    pub blocked: bool,
    pub orphaned: bool,
    pub plan_pending: bool,
    pub stale: bool,
    pub no_evidence: bool,
    pub needs_me: bool,
}

pub fn derive(f: &Facts) -> Derived {
    let todo = f.status == Status::Todo;
    let blocked = todo && f.blockers.iter().any(|s| *s != Status::Done);
    let orphaned = todo && f.blockers.contains(&Status::Dropped);
    let in_progress = f.status == Status::InProgress;
    let plan_pending = in_progress && f.needs_plan_approval && f.has_plan && !f.plan_approved;
    let stale = in_progress && f.claim_stale;
    let review = f.status == Status::Review;
    let no_evidence = review && f.kind == Kind::Code && f.commits == Some(0);
    Derived {
        ready: todo && !blocked,
        blocked,
        orphaned,
        plan_pending,
        stale,
        no_evidence,
        needs_me: f.status == Status::Draft
            || plan_pending
            || stale
            || orphaned
            || review
            || no_evidence,
    }
}

/// Stale rule without an adapter: no activity and no commit for longer than `stale_after`.
pub fn is_stale(
    now: DateTime<Utc>,
    last_activity: DateTime<Utc>,
    last_commit: Option<DateTime<Utc>>,
    stale_after: Duration,
) -> bool {
    let last = last_commit.map_or(last_activity, |c| c.max(last_activity));
    now - last > stale_after
}

#[cfg(test)]
mod tests {
    use super::*;
    use Action::*;
    use Status::*;

    #[test]
    fn transition_table() {
        let cases: &[(Status, Action, Option<Status>)] = &[
            (Draft, Promote, Some(Todo)),
            (Todo, Promote, None),
            (Todo, Claim, Some(InProgress)),
            (Draft, Claim, None),
            (InProgress, Claim, None),
            (Review, Claim, None),
            (InProgress, Steal, Some(InProgress)),
            (Todo, Steal, None),
            (InProgress, Release, Some(Todo)),
            (Review, Release, None),
            (InProgress, Action::Done, Some(Review)),
            (Todo, Action::Done, None),
            (Review, Action::Done, None),
            (Review, Verify, Some(Status::Done)),
            (InProgress, Verify, None),
            (Review, Reject, Some(InProgress)),
            (InProgress, Reject, None),
            (Draft, Drop, Some(Dropped)),
            (Todo, Drop, Some(Dropped)),
            (InProgress, Drop, Some(Dropped)),
            (Review, Drop, Some(Dropped)),
            (Status::Done, Drop, None),
            (Dropped, Drop, None),
            (Status::Done, Reopen, Some(Todo)),
            (Dropped, Reopen, Some(Todo)),
            (Todo, Reopen, None),
            (Review, Reopen, None),
            (InProgress, Approve, Some(InProgress)),
            (Todo, Approve, None),
        ];
        for (from, action, want) in cases {
            let got = transition(*from, *action).ok();
            assert_eq!(got, *want, "{from:?} --{action:?}-->");
        }
    }

    #[test]
    fn bad_transition_names_action_and_status() {
        assert_eq!(
            transition(Status::Done, Drop),
            Err(ModelError::BadTransition {
                action: "drop",
                from: "done"
            })
        );
    }

    #[test]
    fn authorization_table() {
        let all = [
            Promote,
            Claim,
            Steal,
            Release,
            Action::Done,
            Verify,
            Reject,
            Drop,
            Reopen,
            Approve,
        ];
        for a in all {
            for (role, want) in [
                (Role::Human, true),
                (Role::Trusted, matches!(a, Claim | Steal | Action::Done)),
                (Role::Agent, matches!(a, Claim | Action::Done)),
            ] {
                assert_eq!(authorize(a, role).is_ok(), want, "{a:?} as {role:?}");
            }
        }
    }

    #[test]
    fn trust_and_alias_are_human_only() {
        assert!(require_human(Role::Human, "trust").is_ok());
        assert_eq!(
            require_human(Role::Trusted, "trust"),
            Err(ModelError::HumanOnly("trust"))
        );
        assert!(require_human(Role::Agent, "trust").is_err());
    }

    #[test]
    fn new_tasks_start_as_drafts_unless_human_or_trusted() {
        assert_eq!(initial_status(Role::Agent), Draft);
        assert_eq!(initial_status(Role::Trusted), Todo);
        assert_eq!(initial_status(Role::Human), Todo);
    }

    #[test]
    fn done_gate() {
        assert!(check_done_gate(false, false).is_ok());
        assert!(check_done_gate(true, true).is_ok());
        assert_eq!(
            check_done_gate(true, false),
            Err(ModelError::PlanNotApproved)
        );
    }

    fn facts(status: Status, blockers: &[Status]) -> Facts<'_> {
        Facts {
            status,
            kind: Kind::Code,
            needs_plan_approval: false,
            has_plan: false,
            plan_approved: false,
            blockers,
            claim_stale: false,
            commits: None,
        }
    }

    #[test]
    fn ready_and_blocked() {
        let d = derive(&facts(Todo, &[]));
        assert!(d.ready && !d.blocked && !d.needs_me);
        let d = derive(&facts(Todo, &[Status::Done, Status::Done]));
        assert!(d.ready && !d.blocked);
        let d = derive(&facts(Todo, &[Status::Done, InProgress]));
        assert!(!d.ready && d.blocked && !d.orphaned && !d.needs_me);
        for s in [Draft, Review, Status::Done, Dropped, InProgress] {
            let d = derive(&facts(s, &[InProgress]));
            assert!(!d.ready && !d.blocked, "{s:?}");
        }
    }

    #[test]
    fn orphaned_is_blocked_and_needs_me() {
        let d = derive(&facts(Todo, &[Dropped, Status::Done]));
        assert!(d.orphaned && d.blocked && !d.ready && d.needs_me);
    }

    #[test]
    fn plan_pending_rules() {
        let base = Facts {
            needs_plan_approval: true,
            has_plan: true,
            ..facts(InProgress, &[])
        };
        assert!(derive(&base).plan_pending);
        assert!(derive(&base).needs_me);
        assert!(
            !derive(&Facts {
                plan_approved: true,
                ..base.clone()
            })
            .plan_pending
        );
        assert!(
            !derive(&Facts {
                has_plan: false,
                ..base.clone()
            })
            .plan_pending
        );
        assert!(
            !derive(&Facts {
                needs_plan_approval: false,
                ..base.clone()
            })
            .plan_pending
        );
        assert!(
            !derive(&Facts {
                status: Todo,
                ..base
            })
            .plan_pending
        );
    }

    #[test]
    fn stale_only_applies_to_in_progress() {
        let d = derive(&Facts {
            claim_stale: true,
            ..facts(InProgress, &[])
        });
        assert!(d.stale && d.needs_me);
        let d = derive(&Facts {
            claim_stale: true,
            ..facts(Review, &[])
        });
        assert!(!d.stale);
    }

    #[test]
    fn review_needs_me_and_no_evidence() {
        let d = derive(&facts(Review, &[]));
        assert!(d.needs_me && !d.no_evidence);
        let d = derive(&Facts {
            commits: Some(0),
            ..facts(Review, &[])
        });
        assert!(d.needs_me && d.no_evidence);
        let d = derive(&Facts {
            commits: Some(2),
            ..facts(Review, &[])
        });
        assert!(!d.no_evidence);
        let d = derive(&Facts {
            commits: Some(0),
            kind: Kind::Note,
            ..facts(Review, &[])
        });
        assert!(!d.no_evidence && d.needs_me);
        let d = derive(&Facts {
            commits: Some(0),
            ..facts(InProgress, &[])
        });
        assert!(!d.no_evidence);
    }

    #[test]
    fn drafts_need_me_terminal_do_not() {
        assert!(derive(&facts(Draft, &[])).needs_me);
        assert!(!derive(&facts(Status::Done, &[])).needs_me);
        assert!(!derive(&facts(Dropped, &[])).needs_me);
    }

    #[test]
    fn staleness() {
        let now = Utc::now();
        let two_h = Duration::hours(2);
        let ago = |m: i64| now - Duration::minutes(m);
        assert!(is_stale(now, ago(121), None, two_h));
        assert!(!is_stale(now, ago(119), None, two_h));
        // A recent commit keeps the claim fresh even with old fb activity.
        assert!(!is_stale(now, ago(300), Some(ago(30)), two_h));
        // An old commit does not make recent activity stale.
        assert!(!is_stale(now, ago(10), Some(ago(500)), two_h));
        assert!(is_stale(now, ago(300), Some(ago(200)), two_h));
    }

    #[test]
    fn status_and_kind_parse_roundtrip() {
        for s in Status::ALL {
            assert_eq!(Status::parse(s.as_str()), Some(s));
        }
        assert_eq!(Status::parse("nope"), None);
        assert_eq!(Kind::parse("note"), Some(Kind::Note));
        assert_eq!(Kind::parse("x"), None);
    }
}
