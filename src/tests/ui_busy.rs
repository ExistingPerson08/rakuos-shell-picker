use super::Busy;

#[test]
fn a_second_switch_is_refused_while_one_is_running() {
    let busy = Busy::default();
    assert!(busy.claim());
    assert!(!busy.claim(), "a second switch must not start");
    assert!(!busy.claim());
}

#[test]
fn the_switch_can_be_taken_again_after_it_finishes() {
    let busy = Busy::default();
    assert!(busy.claim());
    busy.release();
    assert!(busy.claim());
}

#[test]
fn a_refused_switch_does_not_release_the_one_already_running() {
    // The refused caller must not touch the flag, or a third click would be
    // let through while the first switch is still running.
    let busy = Busy::default();
    assert!(busy.claim());
    assert!(!busy.claim());
    assert!(!busy.claim());
    busy.release();
    assert!(busy.claim());
}
