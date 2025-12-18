use tome_cabi_types::{TomeOwnedStr, TomeStr};

pub fn tome_str(s: &'static str) -> TomeStr {
	TomeStr {
		ptr: s.as_ptr(),
		len: s.len(),
	}
}

pub fn tome_str_to_string(ts: TomeStr) -> String {
	if ts.ptr.is_null() {
		return String::new();
	}
	unsafe {
		let slice = std::slice::from_raw_parts(ts.ptr, ts.len);
		String::from_utf8_lossy(slice).into_owned()
	}
}

pub fn tome_owned_to_string(tos: TomeOwnedStr) -> Option<String> {
	if tos.ptr.is_null() {
		return None;
	}

	unsafe {
		let slice = std::slice::from_raw_parts(tos.ptr, tos.len);
		Some(String::from_utf8_lossy(slice).into_owned())
	}
}

pub fn string_to_tome_owned(s: String) -> TomeOwnedStr {
	let bytes = s.into_bytes().into_boxed_slice();
	let len = bytes.len();
	let ptr = Box::into_raw(bytes) as *mut u8;
	TomeOwnedStr { ptr, len }
}

pub fn plugin_free_str(s: TomeOwnedStr) {
	if s.ptr.is_null() {
		return;
	}

	unsafe {
		let slice = std::ptr::slice_from_raw_parts_mut(s.ptr, s.len);
		drop(Box::from_raw(slice));
	}
}

pub fn strip_ansi_and_controls(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	let mut chars = s.chars().peekable();

	while let Some(ch) = chars.next() {
		if ch == '\u{1b}' {
			if matches!(chars.peek(), Some('[')) {
				let _ = chars.next();
				for c in chars.by_ref() {
					if ('@'..='~').contains(&c) {
						break;
					}
				}
			}
			continue;
		}

		if ch.is_control() {
			continue;
		}

		out.push(ch);
	}

	out
}
