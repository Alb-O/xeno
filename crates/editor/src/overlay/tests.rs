use super::*;

#[test]
fn overlay_store_get_or_default_is_stable_and_mutable() {
	#[derive(Default, Debug, PartialEq)]
	struct Foo {
		n: i32,
	}

	let mut s = OverlayStore {
		inner: Default::default(),
	};

	let p1 = {
		let r = s.get_or_default::<Foo>();
		r.n = 7;
		r as *mut Foo
	};

	let p2 = {
		let r = s.get_or_default::<Foo>();
		assert_eq!(r.n, 7);
		r.n = 9;
		r as *mut Foo
	};

	assert_eq!(p1, p2);

	let r = s.get_or_default::<Foo>();
	assert_eq!(r.n, 9);
}
