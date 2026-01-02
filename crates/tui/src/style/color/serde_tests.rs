//! Tests for serde serialization/deserialization of Color.

use serde::de::{self, Deserialize, IntoDeserializer};

use super::*;

#[test]
fn deserialize() -> Result<(), de::value::Error> {
	assert_eq!(
		Color::Black,
		Color::deserialize("Black".into_deserializer())?
	);
	assert_eq!(
		Color::Magenta,
		Color::deserialize("magenta".into_deserializer())?
	);
	assert_eq!(
		Color::LightGreen,
		Color::deserialize("LightGreen".into_deserializer())?
	);
	assert_eq!(
		Color::White,
		Color::deserialize("bright-white".into_deserializer())?
	);
	assert_eq!(
		Color::Indexed(42),
		Color::deserialize("42".into_deserializer())?
	);
	assert_eq!(
		Color::Rgb(0, 255, 0),
		Color::deserialize("#00ff00".into_deserializer())?
	);
	Ok(())
}

#[test]
fn deserialize_error() {
	let color: Result<_, de::value::Error> = Color::deserialize("invalid".into_deserializer());
	assert!(color.is_err());

	let color: Result<_, de::value::Error> = Color::deserialize("#00000000".into_deserializer());
	assert!(color.is_err());

	let color: Result<Color, _> = serde_json::from_str(r#"{"Rgb":[255,0,255]}"#);
	assert!(color.is_err());
}

#[test]
fn serialize_then_deserialize() -> Result<(), serde_json::Error> {
	let json_rgb = serde_json::to_string(&Color::Rgb(255, 0, 255))?;
	assert_eq!(json_rgb, r##""#FF00FF""##);
	assert_eq!(
		serde_json::from_str::<Color>(&json_rgb)?,
		Color::Rgb(255, 0, 255)
	);

	let json_white = serde_json::to_string(&Color::White)?;
	assert_eq!(json_white, r#""White""#);

	let json_indexed = serde_json::to_string(&Color::Indexed(10))?;
	assert_eq!(json_indexed, r#""10""#);
	assert_eq!(
		serde_json::from_str::<Color>(&json_indexed)?,
		Color::Indexed(10)
	);

	Ok(())
}
