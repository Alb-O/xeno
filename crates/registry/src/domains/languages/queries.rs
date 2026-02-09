use super::registry::LanguageRef;

pub fn get_query_text<'a>(lang: &'a LanguageRef, kind: &str) -> Option<&'a str> {
	let snap = &lang.snap;
	let kind_sym = snap.interner.get(kind)?;

	for q in lang.queries.iter() {
		if q.kind == kind_sym {
			return Some(snap.interner.resolve(q.text));
		}
	}
	None
}
