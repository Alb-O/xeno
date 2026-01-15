//! PKCE (Proof Key for Code Exchange) implementation for OAuth 2.0.
//!
//! Generates cryptographically secure code verifier and challenge pairs
//! using the S256 method as specified in RFC 7636.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// PKCE code verifier and challenge pair.
#[derive(Debug, Clone)]
pub struct PkceCodes {
	/// The code verifier (43-128 chars, URL-safe base64).
	pub verifier: String,
	/// The code challenge (SHA256 hash of verifier, URL-safe base64).
	pub challenge: String,
}

impl PkceCodes {
	/// Generate a new PKCE code pair using S256 method.
	///
	/// The verifier is 86 characters (64 bytes encoded), well within
	/// the 43-128 character requirement of RFC 7636.
	pub fn generate() -> Self {
		let mut bytes = [0u8; 64];
		rand::rng().fill_bytes(&mut bytes);

		let verifier = URL_SAFE_NO_PAD.encode(bytes);
		let digest = Sha256::digest(verifier.as_bytes());
		let challenge = URL_SAFE_NO_PAD.encode(digest);

		Self {
			verifier,
			challenge,
		}
	}
}

/// Generate a cryptographically secure random state parameter.
///
/// Used to prevent CSRF attacks by binding the authorization request
/// to the callback response.
pub fn generate_state() -> String {
	let mut bytes = [0u8; 32];
	rand::rng().fill_bytes(&mut bytes);
	URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn pkce_codes_are_valid_length() {
		let codes = PkceCodes::generate();
		assert!(codes.verifier.len() >= 43 && codes.verifier.len() <= 128);
		assert_eq!(codes.challenge.len(), 43);
	}

	#[test]
	fn pkce_codes_are_url_safe() {
		let codes = PkceCodes::generate();
		let is_url_safe = |s: &str| {
			s.chars()
				.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
		};
		assert!(is_url_safe(&codes.verifier));
		assert!(is_url_safe(&codes.challenge));
	}

	#[test]
	fn pkce_codes_are_unique() {
		let codes1 = PkceCodes::generate();
		let codes2 = PkceCodes::generate();
		assert_ne!(codes1.verifier, codes2.verifier);
		assert_ne!(codes1.challenge, codes2.challenge);
	}

	#[test]
	fn challenge_matches_verifier_hash() {
		let codes = PkceCodes::generate();
		let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(codes.verifier.as_bytes()));
		assert_eq!(codes.challenge, expected);
	}

	#[test]
	fn state_is_valid_length() {
		assert_eq!(generate_state().len(), 43);
	}
}
