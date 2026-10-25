//! Isolation of module panics.
//!
//! The kernel runs module code through `run_guarded`: a panic is caught, and the editor disables the
//! module instead of crashing. [`in_guarded_mode`] lets the runtime's panic hook leave the
//! terminal alone for such panics.
//!
//! Does not help against stack overflows or `abort()`, which end the process.

use std::{
    any::Any,
    cell::Cell,
    panic::{self, AssertUnwindSafe},
};

thread_local! {
    /// Whether module code is running on this thread. Per thread, so that a panic in another
    /// thread (later: background workers) is never mistaken for a module panic.
    static GUARDED: Cell<bool> = const { Cell::new(false) };
}

/// Whether module code is running on this thread right now.
///
/// Meant for a process-wide panic hook: while this is `true`, the panic will be caught and
/// the hook should not restore the terminal or abort.
pub fn in_guarded_mode() -> bool {
    GUARDED.get()
}

/// Runs module code `f`; a panic is caught and returned as its message.
///
/// Sets [`in_guarded_mode`] for the duration of the call and restores the previous value
/// afterwards, also after a panic.
///
/// `AssertUnwindSafe`: module code gets `&mut` access to the state, which is not unwind-safe
/// in general. Every state change a module can make today is a single step, so the state is
/// consistent after a panic; edits will keep this through transactions.
pub(crate) fn run_guarded<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    let previous = GUARDED.replace(true);
    let result = panic::catch_unwind(AssertUnwindSafe(f));
    GUARDED.set(previous);
    result.map_err(|payload| panic_message(payload.as_ref()))
}

/// The message given to `panic!`: it is a `&str` or a `String` (when formatted).
fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_is_set_only_inside_run_guarded() {
        assert!(!in_guarded_mode());
        assert_eq!(run_guarded(in_guarded_mode), Ok(true));
        assert!(!in_guarded_mode());
    }

    #[test]
    fn flag_is_restored_after_a_panic() {
        let _ = run_guarded(|| panic!("x"));
        assert!(!in_guarded_mode());
    }

    #[test]
    fn returns_the_value_when_nothing_panics() {
        assert_eq!(run_guarded(|| 2 + 2), Ok(4));
    }

    #[test]
    fn panic_message_from_str_and_formatted_string() {
        assert_eq!(
            run_guarded(|| panic!("plain")),
            Err::<(), _>("plain".to_string())
        );
        assert_eq!(
            run_guarded(|| panic!("formatted {}", 42)),
            Err::<(), _>("formatted 42".to_string())
        );
    }

    #[test]
    fn non_string_payload_gives_a_generic_message() {
        let result: Result<(), String> = run_guarded(|| std::panic::panic_any(7_u8));
        assert_eq!(result, Err("unknown panic".to_string()));
    }

    #[test]
    fn nested_guard_keeps_the_outer_flag() {
        let inner_after = run_guarded(|| {
            let _ = run_guarded(|| panic!("inner"));
            in_guarded_mode()
        });
        assert_eq!(inner_after, Ok(true));
    }
}
