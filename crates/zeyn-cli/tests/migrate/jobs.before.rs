use zeyn::{ClaimedJob, JobStore, NewJob, SqlJobStore};

fn queued() -> NewJob {
    NewJob {
        name: "mail".into(),
        payload: String::new(),
        run_at_ms: 0,
    }
}

fn inline() -> NewJob { NewJob { name: "a".into() } }

fn empty() -> NewJob { NewJob {} }

fn with_comma() -> NewJob {
    NewJob { name: "a".into(), }
}

fn nested() -> ClaimedJob {
    ClaimedJob {
        id: "1".into(),
        job: NewJob {
            name: "b".into(),
        },
    }
}

fn already() -> NewJob {
    NewJob { name: "a".into(), ..Default::default() }
}

fn based(other: NewJob) -> NewJob {
    NewJob { name: "a".into(), ..other }
}

fn pattern(job: NewJob) -> String {
    let NewJob { name, .. } = job;
    name
}

struct Other { name: String }

fn unrelated() -> Other {
    Other { name: String::new() }
}

struct Store;

impl JobStore for Store {}

impl Clone for Store {}

fn sql(s: &SqlJobStore) {}
