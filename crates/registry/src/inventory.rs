pub struct Reg<T: 'static>(pub &'static T);
pub struct RegSlice<T: 'static>(pub &'static [T]);

macro_rules! collect {
	($t:ty) => {
		::inventory::collect!($crate::inventory::Reg<$t>);
	};
}

macro_rules! collect_slice {
	($t:ty) => {
		::inventory::collect!($crate::inventory::RegSlice<$t>);
	};
}

pub(crate) use {collect, collect_slice};
