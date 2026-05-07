#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_timeline_event_order_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let ts1 = memory.upsert_time_slice("事件当晚", 0, "", "").unwrap();
    let ts2 = memory.upsert_time_slice("三天后", 1, "", "").unwrap();
    memory
        .record_timeline_event(&[1], "confrontation", ts1, "evt1")
        .unwrap();
    memory
        .record_timeline_event(&[1, 2], "reveal", ts2, "evt2")
        .unwrap();
    let events1 = memory.list_timeline_events_by_slice(ts1).unwrap();
    let events2 = memory.list_timeline_events_by_slice(ts2).unwrap();
    let ordered = events1.len() == 1 && events2.len() == 1;
    if !ordered {
        errors.push(format!(
            "timeline event ordering broken: t1={} t2={}",
            events1.len(),
            events2.len()
        ));
    }
    eval_result(
        "writer_agent:timeline_event_order",
        format!(
            "eventsBySlice={} t1={} t2={}",
            ordered,
            events1.len(),
            events2.len()
        ),
        errors,
    )
}
