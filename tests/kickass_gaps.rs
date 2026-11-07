//! What KickAss.jar 5.25 does and R65 doesn't do yet, seen by running the original.
//!
//! Each test states the behaviour of the original. They are ignored because they fail today; see section 22 of
//! docs/R65-Manual.md. Run them with `cargo test --test kickass_gaps -- --ignored`, and remove `#[ignore]` when one is fixed.
mod common;
use common::{fails, Project};

#[test]
#[ignore = "R65 stops at the first error"]
fn all_errors_are_listed() {
    fails("lda foo\nlda bar", "Got 2 errors and 0 warnings while executing:\n  (t.asm 1:5) Error: Unknown symbol 'foo'\n  (t.asm 2:5) Error: Unknown symbol 'bar'");
    fails("foo\nbar", "Got 2 errors and 0 warnings while executing:");
    fails("x: nop\nx: nop\ny: nop\ny: nop", "Got 2 errors and 0 warnings while executing:");
    fails(".var q = 1 +\n2", "Got 2 errors and 0 warnings while executing:");
}

#[test]
#[ignore = "R65 only checks the names in the code it runs"]
fn names_in_code_that_never_runs() {
    fails(".macro m() {\nlda foo\n}", "Unknown symbol 'foo'");
    fails(".if (false) { lda foo }", "Unknown symbol 'foo'");
}

#[test]
#[ignore = "R65 finds the first error of a pass, the original checks names first"]
fn unknown_names_come_before_other_errors() {
    let p = Project::new();
    let r = p.asm("lda foo\nlda #\"x\"", &["-nooutput"]);
    assert!(r.out.contains("Unknown symbol 'foo'"), "{}", r.out);
    assert!(!r.out.contains("flex pass"), "{}", r.out);
}

#[test]
#[ignore = "methods don't count their arguments"]
fn too_many_arguments_for_a_method() {
    fails(".print List().add(1).size(1)", "Invalid number of arguments : size");
}
