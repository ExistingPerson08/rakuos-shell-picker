use super::*;
use std::sync::mpsc;

fn text(line: &Line) -> &str {
    match line {
        Line::Step(t) | Line::Done(t) | Line::Failed(t) => t,
    }
}

/// The bug this module exists for: a dropped sender used to spin here for
/// ever, and the test hangs instead of failing, so it runs on its own thread
/// with a deadline. A green suite is not enough; it has to come back at all.
#[test]
fn a_disconnected_sender_ends_the_wait_promptly() {
    let (tx, rx) = mpsc::channel::<Line>();
    drop(tx);

    let (tx, probe) = mpsc::channel::<(Vec<Line>, bool)>();
    std::thread::spawn(move || {
        let (lines, finished) = receive(&rx);
        let _ = tx.send((lines, finished));
    });

    let (lines, finished) = probe
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("receive() hung on a disconnected channel");
    assert!(finished, "a gone worker means the operation is over");
    assert!(lines.is_empty());
}

#[test]
fn a_live_worker_with_nothing_to_say_is_not_finished() {
    let (tx, rx) = mpsc::channel::<Line>();
    let (lines, finished) = receive(&rx);
    assert!(lines.is_empty());
    assert!(!finished, "the worker is still running, so keep waiting");
    drop(tx);
}

#[test]
fn buffered_steps_are_all_delivered() {
    let (tx, rx) = mpsc::channel::<Line>();
    tx.send(Line::Step("one".into())).unwrap();
    tx.send(Line::Step("two".into())).unwrap();
    let (lines, finished) = receive(&rx);
    assert_eq!(lines.len(), 2);
    assert_eq!(text(&lines[0]), "one");
    assert_eq!(text(&lines[1]), "two");
    assert!(!finished);
    drop(tx);
}

#[test]
fn the_final_line_ends_the_wait_and_leaves_nothing_pending() {
    let (tx, rx) = mpsc::channel::<Line>();
    tx.send(Line::Step("copying".into())).unwrap();
    tx.send(Line::Done("verified".into())).unwrap();
    // A line after the final one is not expected, but if one were ever sent
    // the wait must still end rather than hang on the dropped sender.
    let (lines, finished) = receive(&rx);
    assert!(finished);
    assert_eq!(lines.len(), 2);
    assert_eq!(text(&lines[1]), "verified");
    drop(tx);
}

#[test]
fn a_failure_also_ends_the_wait() {
    let (tx, rx) = mpsc::channel::<Line>();
    tx.send(Line::Failed("disk is full".into())).unwrap();
    let (lines, finished) = receive(&rx);
    assert!(finished, "a failed operation must release the panel");
    assert_eq!(text(&lines[0]), "disk is full");
}

#[test]
fn repeated_drains_after_the_end_stay_finished() {
    // The GTK timeout may fire once more after the last line, and the source
    // is only removed once it returns Break. It must not start spinning.
    let (tx, rx) = mpsc::channel::<Line>();
    tx.send(Line::Done("done".into())).unwrap();
    drop(tx);
    for _ in 0..5 {
        let (_, finished) = receive(&rx);
        assert!(finished);
    }
}
