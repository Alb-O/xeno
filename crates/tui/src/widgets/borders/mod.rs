//! Border related types ([`Borders`], [`BorderType`]) and a macro to create borders ([`border`]).
use core::fmt;

use bitflags::bitflags;

use crate::symbols::border;

bitflags! {
	/// Bitflags that can be composed to set the visible borders essentially on the block widget.
	#[derive(Default, Clone, Copy, Eq, PartialEq, Hash)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct Borders: u8 {
		/// Show the top border
		const TOP    = 0b0001;
		/// Show the right border
		const RIGHT  = 0b0010;
		/// Show the bottom border
		const BOTTOM = 0b0100;
		/// Show the left border
		const LEFT   = 0b1000;
		/// Show all borders
		const ALL = Self::TOP.bits() | Self::RIGHT.bits() | Self::BOTTOM.bits() | Self::LEFT.bits();
	}
}

impl Borders {
	/// Show no border (default)
	pub const NONE: Self = Self::empty();
}

/// The type of border of a [`Block`](crate::widgets::block::Block).
///
/// See the [`borders`](crate::widgets::block::Block::borders) method of `Block` to configure its
/// borders.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BorderType {
	/// A plain, simple border.
	///
	/// # Example
	///
	/// ```plain
	/// ┌───────┐
	/// │       │
	/// └───────┘
	/// ```
	Plain,
	/// A plain border with rounded corners.
	///
	/// # Example
	///
	/// ```plain
	/// ╭───────╮
	/// │       │
	/// ╰───────╯
	/// ```
	Rounded,
	/// A doubled border.
	///
	/// Note this uses one character that draws two lines.
	///
	/// # Example
	///
	/// ```plain
	/// ╔═══════╗
	/// ║       ║
	/// ╚═══════╝
	/// ```
	Double,
	/// A thick border.
	///
	/// # Example
	///
	/// ```plain
	/// ┏━━━━━━━┓
	/// ┃       ┃
	/// ┗━━━━━━━┛
	/// ```
	Thick,
	/// A light double-dashed border.
	///
	/// ```plain
	/// ┌╌╌╌╌╌╌╌┐
	/// ╎       ╎
	/// └╌╌╌╌╌╌╌┘
	/// ```
	LightDoubleDashed,
	/// A heavy double-dashed border.
	///
	/// ```plain
	/// ┏╍╍╍╍╍╍╍┓
	/// ╏       ╏
	/// ┗╍╍╍╍╍╍╍┛
	/// ```
	HeavyDoubleDashed,
	/// A light triple-dashed border.
	///
	/// ```plain
	/// ┌┄┄┄┄┄┄┄┐
	/// ┆       ┆
	/// └┄┄┄┄┄┄┄┘
	/// ```
	LightTripleDashed,
	/// A heavy triple-dashed border.
	///
	/// ```plain
	/// ┏┅┅┅┅┅┅┅┓
	/// ┇       ┇
	/// ┗┅┅┅┅┅┅┅┛
	/// ```
	HeavyTripleDashed,
	/// A light quadruple-dashed border.
	///
	/// ```plain
	/// ┌┈┈┈┈┈┈┈┐
	/// ┊       ┊
	/// └┈┈┈┈┈┈┈┘
	/// ```
	LightQuadrupleDashed,
	/// A heavy quadruple-dashed border.
	///
	/// ```plain
	/// ┏┉┉┉┉┉┉┉┓
	/// ┋       ┋
	/// ┗┉┉┉┉┉┉┉┛
	/// ```
	HeavyQuadrupleDashed,
	/// A border with a single line on the inside of a half block.
	///
	/// # Example
	///
	/// ```plain
	/// ▗▄▄▄▄▄▄▄▖
	/// ▐       ▌
	/// ▐       ▌
	/// ▝▀▀▀▀▀▀▀▘
	QuadrantInside,

	/// A border with a single line on the outside of a half block.
	///
	/// # Example
	///
	/// ```plain
	/// ▛▀▀▀▀▀▀▀▜
	/// ▌       ▐
	/// ▌       ▐
	/// ▙▄▄▄▄▄▄▄▟
	QuadrantOutside,
	/// A border that uses only spaces, effectively creating padding that uses the block's
	/// background style.
	///
	/// This is the default border type, providing a minimal visual separation without
	/// box-drawing characters.
	///
	/// # Example
	///
	/// ```plain
	///  xxxxxxx
	///  x     x
	///  xxxxxxx
	/// ```
	#[default]
	Padded,
	/// A border with a solid colored stripe on the left edge only.
	///
	/// This style is useful for notification toasts and callout boxes where a thin accent bar
	/// on the left indicates the notification level or type. The left edge uses a full block
	/// character that can be styled with a foreground color.
	///
	/// # Example
	///
	/// ```plain
	/// █xxxxxxx
	/// █x     x
	/// █xxxxxxx
	/// ```
	Stripe,
}

impl BorderType {
	/// Convert this `BorderType` into the corresponding [`Set`](border::Set) of border symbols.
	pub const fn border_symbols<'a>(border_type: Self) -> border::Set<'a> {
		match border_type {
			Self::Plain => border::PLAIN,
			Self::Rounded => border::ROUNDED,
			Self::Double => border::DOUBLE,
			Self::Thick => border::THICK,
			Self::LightDoubleDashed => border::LIGHT_DOUBLE_DASHED,
			Self::HeavyDoubleDashed => border::HEAVY_DOUBLE_DASHED,
			Self::LightTripleDashed => border::LIGHT_TRIPLE_DASHED,
			Self::HeavyTripleDashed => border::HEAVY_TRIPLE_DASHED,
			Self::LightQuadrupleDashed => border::LIGHT_QUADRUPLE_DASHED,
			Self::HeavyQuadrupleDashed => border::HEAVY_QUADRUPLE_DASHED,
			Self::QuadrantInside => border::QUADRANT_INSIDE,
			Self::QuadrantOutside => border::QUADRANT_OUTSIDE,
			Self::Padded => border::EMPTY,
			Self::Stripe => border::STRIPE,
		}
	}

	/// Convert this `BorderType` into the corresponding [`Set`](border::Set) of border symbols.
	pub const fn to_border_set<'a>(self) -> border::Set<'a> {
		Self::border_symbols(self)
	}
}

impl fmt::Display for BorderType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Plain => write!(f, "Plain"),
			Self::Rounded => write!(f, "Rounded"),
			Self::Double => write!(f, "Double"),
			Self::Thick => write!(f, "Thick"),
			Self::LightDoubleDashed => write!(f, "LightDoubleDashed"),
			Self::HeavyDoubleDashed => write!(f, "HeavyDoubleDashed"),
			Self::LightTripleDashed => write!(f, "LightTripleDashed"),
			Self::HeavyTripleDashed => write!(f, "HeavyTripleDashed"),
			Self::LightQuadrupleDashed => write!(f, "LightQuadrupleDashed"),
			Self::HeavyQuadrupleDashed => write!(f, "HeavyQuadrupleDashed"),
			Self::QuadrantInside => write!(f, "QuadrantInside"),
			Self::QuadrantOutside => write!(f, "QuadrantOutside"),
			Self::Padded => write!(f, "Padded"),
			Self::Stripe => write!(f, "Stripe"),
		}
	}
}

impl std::str::FromStr for BorderType {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Plain" => Ok(Self::Plain),
			"Rounded" => Ok(Self::Rounded),
			"Double" => Ok(Self::Double),
			"Thick" => Ok(Self::Thick),
			"LightDoubleDashed" => Ok(Self::LightDoubleDashed),
			"HeavyDoubleDashed" => Ok(Self::HeavyDoubleDashed),
			"LightTripleDashed" => Ok(Self::LightTripleDashed),
			"HeavyTripleDashed" => Ok(Self::HeavyTripleDashed),
			"LightQuadrupleDashed" => Ok(Self::LightQuadrupleDashed),
			"HeavyQuadrupleDashed" => Ok(Self::HeavyQuadrupleDashed),
			"QuadrantInside" => Ok(Self::QuadrantInside),
			"QuadrantOutside" => Ok(Self::QuadrantOutside),
			"Padded" => Ok(Self::Padded),
			"Stripe" => Ok(Self::Stripe),
			_ => Err(format!("unknown variant: {s}")),
		}
	}
}

impl fmt::Debug for Borders {
	/// Display the Borders bitflags as a list of names.
	///
	/// `Borders::NONE` is displayed as `NONE` and `Borders::ALL` is displayed as `ALL`. If multiple
	/// flags are set, they are otherwise displayed separated by a pipe character.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.is_empty() {
			return write!(f, "NONE");
		}
		if self.is_all() {
			return write!(f, "ALL");
		}
		let mut names = self.iter_names().map(|(name, _)| name);
		if let Some(first) = names.next() {
			write!(f, "{first}")?;
		}
		for name in names {
			write!(f, " | {name}")?;
		}
		Ok(())
	}
}

/// Macro that constructs and returns a combination of the [`Borders`] object from TOP, BOTTOM, LEFT
/// and RIGHT.
///
/// When used with NONE you should consider omitting this completely. For ALL you should consider
/// [`Block::bordered()`](crate::widgets::block::Block::bordered) instead.
///
/// ## Examples
///
/// ```
/// use xeno_tui::border;
/// use xeno_tui::widgets::Block;
///
/// Block::new()
///     .title("Construct Borders and use them in place")
///     .borders(border!(TOP, BOTTOM));
/// ```
///
/// `border!` can be called with any number of individual sides:
///
/// ```
/// use xeno_tui::border;
/// use xeno_tui::widgets::Borders;
/// let right_open = border!(TOP, LEFT, BOTTOM);
/// assert_eq!(right_open, Borders::TOP | Borders::LEFT | Borders::BOTTOM);
/// ```
///
/// Single borders work but using `Borders::` directly would be simpler.
///
/// ```
/// use xeno_tui::border;
/// use xeno_tui::widgets::Borders;
///
/// assert_eq!(border!(TOP), Borders::TOP);
/// assert_eq!(border!(ALL), Borders::ALL);
/// assert_eq!(border!(), Borders::NONE);
/// ```
#[macro_export]
macro_rules! border {
    () => {
        $crate::widgets::borders::Borders::NONE
    };
    ($b:ident) => {
        $crate::widgets::borders::Borders::$b
    };
    ($first:ident,$($other:ident),*) => {
        $crate::widgets::borders::Borders::$first
        $(
            .union($crate::widgets::borders::Borders::$other)
        )*
    };
}

#[cfg(test)]
mod tests;
