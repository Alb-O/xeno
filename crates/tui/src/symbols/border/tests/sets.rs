//! Tests for individual border set rendering.

use indoc::indoc;

use super::*;

#[test]
fn plain() {
	assert_eq!(
		render(PLAIN),
		indoc!(
			"░░░░░░
             ░┌──┐░
             ░│░░│░
             ░│░░│░
             ░└──┘░
             ░░░░░░"
		)
	);
}

#[test]
fn rounded() {
	assert_eq!(
		render(ROUNDED),
		indoc!(
			"░░░░░░
             ░╭──╮░
             ░│░░│░
             ░│░░│░
             ░╰──╯░
             ░░░░░░"
		)
	);
}

#[test]
fn double() {
	assert_eq!(
		render(DOUBLE),
		indoc!(
			"░░░░░░
             ░╔══╗░
             ░║░░║░
             ░║░░║░
             ░╚══╝░
             ░░░░░░"
		)
	);
}

#[test]
fn thick() {
	assert_eq!(
		render(THICK),
		indoc!(
			"░░░░░░
             ░┏━━┓░
             ░┃░░┃░
             ░┃░░┃░
             ░┗━━┛░
             ░░░░░░"
		)
	);
}

#[test]
fn light_double_dashed() {
	assert_eq!(
		render(LIGHT_DOUBLE_DASHED),
		indoc!(
			"░░░░░░
             ░┌╌╌┐░
             ░╎░░╎░
             ░╎░░╎░
             ░└╌╌┘░
             ░░░░░░"
		)
	);
}

#[test]
fn heavy_double_dashed() {
	assert_eq!(
		render(HEAVY_DOUBLE_DASHED),
		indoc!(
			"░░░░░░
             ░┏╍╍┓░
             ░╏░░╏░
             ░╏░░╏░
             ░┗╍╍┛░
             ░░░░░░"
		)
	);
}

#[test]
fn light_triple_dashed() {
	assert_eq!(
		render(LIGHT_TRIPLE_DASHED),
		indoc!(
			"░░░░░░
             ░┌┄┄┐░
             ░┆░░┆░
             ░┆░░┆░
             ░└┄┄┘░
             ░░░░░░"
		)
	);
}

#[test]
fn heavy_triple_dashed() {
	assert_eq!(
		render(HEAVY_TRIPLE_DASHED),
		indoc!(
			"░░░░░░
             ░┏┅┅┓░
             ░┇░░┇░
             ░┇░░┇░
             ░┗┅┅┛░
             ░░░░░░"
		)
	);
}

#[test]
fn light_quadruple_dashed() {
	assert_eq!(
		render(LIGHT_QUADRUPLE_DASHED),
		indoc!(
			"░░░░░░
             ░┌┈┈┐░
             ░┊░░┊░
             ░┊░░┊░
             ░└┈┈┘░
             ░░░░░░"
		)
	);
}

#[test]
fn heavy_quadruple_dashed() {
	assert_eq!(
		render(HEAVY_QUADRUPLE_DASHED),
		indoc!(
			"░░░░░░
             ░┏┉┉┓░
             ░┋░░┋░
             ░┋░░┋░
             ░┗┉┉┛░
             ░░░░░░"
		)
	);
}

#[test]
fn quadrant_outside() {
	assert_eq!(
		render(QUADRANT_OUTSIDE),
		indoc!(
			"░░░░░░
             ░▛▀▀▜░
             ░▌░░▐░
             ░▌░░▐░
             ░▙▄▄▟░
             ░░░░░░"
		)
	);
}

#[test]
fn quadrant_inside() {
	assert_eq!(
		render(QUADRANT_INSIDE),
		indoc!(
			"░░░░░░
             ░▗▄▄▖░
             ░▐░░▌░
             ░▐░░▌░
             ░▝▀▀▘░
             ░░░░░░"
		)
	);
}

#[test]
fn one_eighth_wide() {
	assert_eq!(
		render(ONE_EIGHTH_WIDE),
		indoc!(
			"░░░░░░
             ░▁▁▁▁░
             ░▏░░▕░
             ░▏░░▕░
             ░▔▔▔▔░
             ░░░░░░"
		)
	);
}

#[test]
fn one_eighth_tall() {
	assert_eq!(
		render(ONE_EIGHTH_TALL),
		indoc!(
			"░░░░░░
             ░▕▔▔▏░
             ░▕░░▏░
             ░▕░░▏░
             ░▕▁▁▏░
             ░░░░░░"
		)
	);
}

#[test]
fn proportional_wide() {
	assert_eq!(
		render(PROPORTIONAL_WIDE),
		indoc!(
			"░░░░░░
             ░▄▄▄▄░
             ░█░░█░
             ░█░░█░
             ░▀▀▀▀░
             ░░░░░░"
		)
	);
}

#[test]
fn proportional_tall() {
	assert_eq!(
		render(PROPORTIONAL_TALL),
		indoc!(
			"░░░░░░
             ░█▀▀█░
             ░█░░█░
             ░█░░█░
             ░█▄▄█░
             ░░░░░░"
		)
	);
}

#[test]
fn full() {
	assert_eq!(
		render(FULL),
		indoc!(
			"░░░░░░
             ░████░
             ░█░░█░
             ░█░░█░
             ░████░
             ░░░░░░"
		)
	);
}

#[test]
fn empty() {
	assert_eq!(
		render(EMPTY),
		indoc!(
			"░░░░░░
             ░    ░
             ░ ░░ ░
             ░ ░░ ░
             ░    ░
             ░░░░░░"
		)
	);
}

#[test]
fn stripe() {
	assert_eq!(
		render(STRIPE),
		indoc!(
			"░░░░░░
             ░▏   ░
             ░▏░░ ░
             ░▏░░ ░
             ░▏   ░
             ░░░░░░"
		)
	);
}
