#[cold]
#[track_caller]
pub(crate) fn unknown(kind: &'static str, val: &str) -> ! {
	panic!("unknown {kind}: '{val}'")
}

#[cfg(feature = "actions")]
pub(crate) mod actions;
#[cfg(feature = "actions")]
pub(crate) use actions::*;

#[cfg(feature = "options")]
pub(crate) mod options;
#[cfg(feature = "options")]
pub(crate) use options::*;
