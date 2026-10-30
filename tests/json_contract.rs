//! Snapshots of the agent-facing `--json` output, so the format cannot change by accident.
//! Volatile values (timestamps, temp paths, hashes, host, actor labels) are redacted.

mod common;

use common::{Env, Run, Who};
use serde_json::Value;

const A: Who = Who::Agent("agent-aaaaaaa");
const B: Who = Who::Agent("agent-bbbbbbb");
const H: Who = Who::Human;

fn is_timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 20
        && b[4] == b'-'
        && b[10] == b'T'
        && b[19] == b'Z'
        && s[..4].bytes().all(|c| c.is_ascii_digit())
}

fn is_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|c| c.is_ascii_hexdigit())
}

fn collect_labels(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if k == "label"
                    && let Some(s) = x.as_str()
                {
                    out.push(s.to_string());
                }
                collect_labels(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| collect_labels(x, out)),
        _ => {}
    }
}

fn redact(v: &mut Value, subs: &[(String, &str)]) {
    match v {
        Value::String(s) => {
            if is_timestamp(s) {
                *s = "<ts>".into();
            } else if is_sha(s) {
                *s = "<sha>".into();
            } else {
                for (from, to) in subs {
                    if s.contains(from.as_str()) {
                        *s = s.replace(from.as_str(), to);
                    }
                }
            }
        }
        Value::Number(_) => {}
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                if k == "age_seconds" {
                    *x = Value::from(0);
                } else {
                    redact(x, subs);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| redact(x, subs)),
        _ => {}
    }
}

fn snap(e: &Env, r: &Run) -> String {
    let mut v = r.json();
    // Actor labels come from the identity module; redact them everywhere, messages included.
    let mut labels: Vec<String> = [A, B]
        .iter()
        .map(|w| {
            e.run(*w, &["prime"]).data()["actor"]["label"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    collect_labels(&v, &mut labels);
    labels.sort_by_key(|l| std::cmp::Reverse(l.len()));
    labels.dedup();
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    let mut subs: Vec<(String, &str)> = vec![
        (e.work_path.to_string_lossy().into_owned(), "<work>"),
        (e.home.path().to_string_lossy().into_owned(), "<home>"),
    ];
    subs.extend(labels.into_iter().map(|l| (l, "<label>")));
    subs.push((host, "<host>"));
    redact(&mut v, &subs);
    // Host is a plain field too.
    fn host_fields(v: &mut Value) {
        match v {
            Value::Object(m) => {
                for (k, x) in m.iter_mut() {
                    if k == "host" {
                        *x = Value::from("<host>");
                    } else {
                        host_fields(x);
                    }
                }
            }
            Value::Array(a) => a.iter_mut().for_each(host_fields),
            _ => {}
        }
    }
    host_fields(&mut v);
    serde_json::to_string_pretty(&v).unwrap() + "\n"
}

#[test]
fn contract_snapshots() {
    let e = Env::new();

    let r = e.run(
        A,
        &[
            "add",
            "Rate-limit public endpoints",
            "--goal",
            "Protect the API",
            "--ac",
            "429 after 100 req/min",
            "--prio",
            "1",
            "--ticket",
            "jira:ABC-36",
            "--ticket-title",
            "Hardening",
        ],
    );
    insta::assert_snapshot!("add_agent_draft", snap(&e, &r));

    let r = e.run(H, &["promote", "fb-1"]);
    insta::assert_snapshot!("promote", snap(&e, &r));

    e.run(H, &["add", "Follow-up", "--blocked-by", "fb-1"]);
    let r = e.run(H, &["ready"]);
    insta::assert_snapshot!("ready", snap(&e, &r));

    let r = e.run(A, &["claim"]);
    insta::assert_snapshot!("claim", snap(&e, &r));

    e.run(A, &["plan", "fb-1", "Add a token bucket."]);
    e.run(
        A,
        &[
            "note",
            "fb-1",
            "Bucket done, tests next.",
            "--kind",
            "handoff",
        ],
    );
    e.git(&["checkout", "-q", "-b", "feature"]);
    e.write("limit.rs", "// limiter\n");
    e.commit("add limiter");

    let r = e.run(A, &["done", "fb-1"]);
    insta::assert_snapshot!("done", snap(&e, &r));

    let r = e.run(H, &["show", "fb-1", "--evidence"]);
    insta::assert_snapshot!("show_with_evidence", snap(&e, &r));

    let r = e.run(H, &["list", "--needs-me"]);
    insta::assert_snapshot!("list_needs_me", snap(&e, &r));

    let r = e.run(B, &["prime"]);
    insta::assert_snapshot!("prime_no_claim", snap(&e, &r));

    let r = e.run(A, &["prime"]);
    insta::assert_snapshot!("prime_with_claim", snap(&e, &r));

    let r = e.run(A, &["promote", "fb-2"]);
    insta::assert_snapshot!("error_human_only", snap(&e, &r));

    let r = e.run(B, &["claim", "fb-1"]);
    insta::assert_snapshot!("error_claim_held", snap(&e, &r));

    let r = e.run(H, &["show", "fb-99"]);
    insta::assert_snapshot!("error_not_found", snap(&e, &r));

    let r = e.run(H, &["verify", "fb-1"]);
    insta::assert_snapshot!("verify", snap(&e, &r));

    let r = e.run(A, &["whoami"]);
    insta::assert_snapshot!("whoami", snap(&e, &r));
}
