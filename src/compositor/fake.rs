//! A compositor that is registered nowhere, for tests that need a second one.
//!
//! Every rule about not touching the wrong compositor needs two compositors to be
//! a rule at all. Niri used to be the second one, and registering it for that
//! purpose is what put a tab on screen that could not complete a single switch.
//!
//! These tests instead use a compositor that exists only in the test build. It is
//! never in [`crate::compositor::all`], so it cannot reach a user's tab bar, and
//! it keeps the coverage for the moment a real second compositor is added —
//! which is the whole reason to keep that code rather than deleting it as
//! unreachable.
//!
//! Its id is `"fake"`, which is not a compositor anyone runs. Detection can never
//! name it, because [`crate::compositor::detect_with`] only considers registered
//! compositors. Tests that need the *reverse* case, a foreign compositor being
//! the running one, build a [`crate::compositor::Detection`] by hand instead.

use crate::compositor::{Compositor, EnvSource};
use crate::shell::Shell;

pub struct Fake;

impl Compositor for Fake {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn label(&self) -> &'static str {
        "fake"
    }

    fn shells(&self) -> &'static [Shell] {
        &[]
    }

    fn config_dir(&self) -> &'static str {
        ".config/fake"
    }

    fn marker_env(&self) -> &'static str {
        "RSP_FAKE_SIGNATURE"
    }

    fn is_running_with(&self, env: &dyn EnvSource) -> bool {
        env.var(self.marker_env()).is_some()
    }

    // Nothing to reload: the point of the fake is to be a second registrant,
    // not a second compositor with a configuration of its own.
    fn reload(&self) -> Result<(), String> {
        Ok(())
    }
}

pub static FAKE: Fake = Fake;
