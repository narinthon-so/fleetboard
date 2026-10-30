//! `fb prime`: the workflow guide an agent reads at session start.

use chrono::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanState {
    /// The task does not need plan approval.
    NotRequired,
    Pending,
    Approved,
}

#[derive(Debug, Clone)]
pub struct Handoff {
    pub author: String,
    pub age: Duration,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct PrimeClaim {
    /// `fb-12`
    pub id: String,
    pub title: String,
    pub status: String,
    pub plan: PlanState,
    pub handoff: Option<Handoff>,
}

#[derive(Debug, Clone)]
pub struct PrimeInput {
    pub label: String,
    pub host: String,
    pub project: Option<String>,
    pub claims: Vec<PrimeClaim>,
}

/// "just now", "5m ago", "2h ago", "3d ago".
pub fn ago(d: Duration) -> String {
    let m = d.num_minutes();
    match m {
        ..1 => "just now".to_string(),
        1..60 => format!("{m}m ago"),
        60..1440 => format!("{}h ago", m / 60),
        _ => format!("{}d ago", m / 1440),
    }
}

pub fn render(p: &PrimeInput) -> String {
    let mut out = format!(
        "fleetboard — you are {} on host {}, project {}\n",
        p.label,
        p.host,
        p.project.as_deref().unwrap_or("(none: not in a git repo)")
    );
    for c in &p.claims {
        let plan = match c.plan {
            PlanState::NotRequired => "",
            PlanState::Pending => ", plan awaiting approval",
            PlanState::Approved => ", plan approved",
        };
        out.push_str(&format!(
            "Your claim: {} \"{}\" ({}{plan})\n",
            c.id, c.title, c.status
        ));
        if let Some(h) = &c.handoff {
            out.push_str(&format!("Last handoff ({}, {}):\n", h.author, ago(h.age)));
            for line in h.body.lines() {
                out.push_str(&format!("  {line}\n"));
            }
        }
    }
    match p.claims.first() {
        Some(c) => out.push_str(&format!(
            "Workflow: fb show {id} · fb note {id} \"…\" (handoff before you stop) · fb done {id}\n",
            id = c.id
        )),
        None => out.push_str("You have no active claim.\n"),
    }
    out.push_str(
        "No claim? fb ready, then fb claim <id>. New work you find: fb add \"…\" (lands as a draft).\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(claims: Vec<PrimeClaim>) -> PrimeInput {
        PrimeInput {
            label: "codex 403254b".into(),
            host: "RPI".into(),
            project: Some("github.com/org/repo".into()),
            claims,
        }
    }

    #[test]
    fn matches_the_design_example() {
        let text = render(&input(vec![PrimeClaim {
            id: "fb-12".into(),
            title: "Rate-limit public endpoints".into(),
            status: "in_progress".into(),
            plan: PlanState::Approved,
            handoff: Some(Handoff {
                author: "claude 6df8aed".into(),
                age: Duration::hours(2),
                body: "line one\nline two".into(),
            }),
        }]));
        assert_eq!(
            text,
            "fleetboard — you are codex 403254b on host RPI, project github.com/org/repo\n\
             Your claim: fb-12 \"Rate-limit public endpoints\" (in_progress, plan approved)\n\
             Last handoff (claude 6df8aed, 2h ago):\n  line one\n  line two\n\
             Workflow: fb show fb-12 · fb note fb-12 \"…\" (handoff before you stop) · fb done fb-12\n\
             No claim? fb ready, then fb claim <id>. New work you find: fb add \"…\" (lands as a draft).\n"
        );
    }

    #[test]
    fn without_a_claim() {
        let mut i = input(vec![]);
        i.project = None;
        let text = render(&i);
        assert!(text.contains("project (none: not in a git repo)"));
        assert!(text.contains("You have no active claim."));
        assert!(!text.contains("Your claim"));
    }

    #[test]
    fn age_buckets() {
        assert_eq!(ago(Duration::seconds(10)), "just now");
        assert_eq!(ago(Duration::minutes(5)), "5m ago");
        assert_eq!(ago(Duration::minutes(59)), "59m ago");
        assert_eq!(ago(Duration::hours(2)), "2h ago");
        assert_eq!(ago(Duration::days(3)), "3d ago");
        assert_eq!(ago(Duration::seconds(-30)), "just now");
    }
}
