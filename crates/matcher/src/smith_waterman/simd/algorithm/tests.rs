use super::*;
use crate::r#const::*;

const CHAR_SCORE: u16 = MATCH_SCORE + MATCHING_CASE_BONUS;

fn get_score(needle: &str, haystack: &str) -> u16 {
	smith_waterman::<16, 1>(needle, &[haystack], None, &Scoring::default()).0[0]
}

#[test]
fn test_score_basic() {
	assert_eq!(get_score("b", "abc"), CHAR_SCORE);
	assert_eq!(get_score("c", "abc"), CHAR_SCORE);
}

#[test]
fn test_score_prefix() {
	assert_eq!(get_score("a", "abc"), CHAR_SCORE + PREFIX_BONUS);
	assert_eq!(get_score("a", "aabc"), CHAR_SCORE + PREFIX_BONUS);
	assert_eq!(get_score("a", "babc"), CHAR_SCORE);
}

#[test]
fn test_score_offset_prefix() {
	// Give prefix bonus on second char if the first char isn't a letter
	assert_eq!(get_score("a", "-a"), CHAR_SCORE + OFFSET_PREFIX_BONUS);
	assert_eq!(get_score("-a", "-ab"), 2 * CHAR_SCORE + PREFIX_BONUS);
	assert_eq!(get_score("a", "'a"), CHAR_SCORE + OFFSET_PREFIX_BONUS);
	assert_eq!(get_score("a", "Ba"), CHAR_SCORE);
}

#[test]
fn test_score_exact_match() {
	assert_eq!(get_score("a", "a"), CHAR_SCORE + EXACT_MATCH_BONUS + PREFIX_BONUS);
	assert_eq!(get_score("abc", "abc"), 3 * CHAR_SCORE + EXACT_MATCH_BONUS + PREFIX_BONUS);
}

#[test]
fn test_score_delimiter() {
	assert_eq!(get_score("-", "a--bc"), CHAR_SCORE);
	assert_eq!(get_score("b", "a-b"), CHAR_SCORE + DELIMITER_BONUS);
	assert_eq!(get_score("a", "a-b-c"), CHAR_SCORE + PREFIX_BONUS);
	assert_eq!(get_score("b", "a--b"), CHAR_SCORE + DELIMITER_BONUS);
	assert_eq!(get_score("c", "a--bc"), CHAR_SCORE);
	assert_eq!(get_score("a", "-a--bc"), CHAR_SCORE + OFFSET_PREFIX_BONUS);
}

#[test]
fn test_score_no_delimiter_for_delimiter_chars() {
	assert_eq!(get_score("-", "a-bc"), CHAR_SCORE);
	assert_eq!(get_score("-", "a--bc"), CHAR_SCORE);
	assert!(get_score("a_b", "a_bb") > get_score("a_b", "a__b"));
}

#[test]
fn test_score_affine_gap() {
	assert_eq!(get_score("test", "Uterst"), CHAR_SCORE * 4 - GAP_OPEN_PENALTY);
	assert_eq!(get_score("test", "Uterrst"), CHAR_SCORE * 4 - GAP_OPEN_PENALTY - GAP_EXTEND_PENALTY);
}

#[test]
fn test_score_capital_bonus() {
	assert_eq!(get_score("a", "A"), MATCH_SCORE + PREFIX_BONUS);
	assert_eq!(get_score("A", "Aa"), CHAR_SCORE + PREFIX_BONUS);
	assert_eq!(get_score("D", "forDist"), CHAR_SCORE + CAPITALIZATION_BONUS);
	assert_eq!(get_score("D", "foRDist"), CHAR_SCORE);
	assert_eq!(get_score("D", "FOR_DIST"), CHAR_SCORE + DELIMITER_BONUS);
}

#[test]
fn test_score_prefix_beats_delimiter() {
	assert!(get_score("swap", "swap(test)") > get_score("swap", "iter_swap(test)"));
	assert!(get_score("_", "_private_member") > get_score("_", "public_member"));
}

#[test]
fn test_score_prefix_beats_capitalization() {
	assert!(get_score("H", "HELLO") > get_score("H", "fooHello"));
}

#[test]
fn test_score_continuous_beats_delimiter() {
	assert!(get_score("foo", "fooo") > get_score("foo", "f_o_o_o"));
}

#[test]
fn test_score_continuous_beats_capitalization() {
	assert!(get_score("fo", "foo") > get_score("fo", "faOo"));
}
