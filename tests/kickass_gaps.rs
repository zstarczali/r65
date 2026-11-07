//! What KickAss.jar 5.25 does and R65 doesn't do yet, seen by running the original.
//!
//! Each test states the behaviour of the original. They are ignored because they fail today; see section 22 of
//! docs/R65-Manual.md. Run them with `cargo test --test kickass_gaps -- --ignored`, and remove `#[ignore]` when one is fixed.
mod common;
use common::{Project, fails};

#[test]
#[ignore = "R65 continues differently after an error on a line of another assembler's code"]
fn more_errors_on_a_line_that_starts_with_a_word_label() {
    fails("!macro m {\n}", "1:7) Error: Invalid token");
}

#[test]
#[ignore = "R65 reports a syntax error at the end of the file"]
fn an_open_bracket_before_the_end_of_an_instruction() {
    fails("lda (\nnop\n", "Unknown symbol 'nop'");
}

#[test]
#[ignore = "R65 doesn't count the column after an unterminated string like the original"]
fn the_line_after_an_unterminated_string() {
    fails(".text \"abc\n.byte +\n", "2:8) Error: Syntax error");
}

#[test]
#[ignore = "R65 checks the whole block against the limits"]
fn an_overlapped_part_of_a_block_is_not_out_of_bounds() {
    fails(".segment D [start=$1000, max=$1000]\n.byte 1,2\n*=$1000 \"b\"\n.byte 3", "'D' ($1001-$1001) is out of segment bounds");
}

#[test]
#[ignore = "R65 first reports the name"]
fn return_runs_before_the_end_of_a_define() {
    fails(".define q { .return 1\n.var q=1 }\n", "Uncaught return statement");
}

#[test]
#[ignore = "R65 runs one more pass"]
fn a_name_missing_from_a_later_namespace_stops_after_one_pass() {
    let p = Project::new();
    let r = p.asm("lda s.v\n.namespace s {\n.var v=1\n}\n", &["-nooutput"]);
    assert!(!r.has_line("flex pass 2"), "{}", r.out);
}
