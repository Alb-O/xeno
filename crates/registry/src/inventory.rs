/// Dead: unused throughout the workspace.
pub struct Reg<T: 'static>(pub &'static T);
pub struct RegSlice<T: 'static>(pub &'static [T]);
