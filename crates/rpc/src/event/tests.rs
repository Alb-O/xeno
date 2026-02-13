use super::*;

#[test]
fn any_event() {
	#[derive(Debug, Clone, PartialEq, Eq)]
	struct MyEvent<T>(T);

	let event = MyEvent("hello".to_owned());
	let mut any_event = AnyEvent::new(event.clone());
	assert!(any_event.type_name().contains("MyEvent"));

	assert!(!any_event.is::<String>());
	assert!(!any_event.is::<MyEvent<i32>>());
	assert!(any_event.is::<MyEvent<String>>());

	assert_eq!(any_event.downcast_ref::<i32>(), None);
	assert_eq!(any_event.downcast_ref::<MyEvent<String>>(), Some(&event));

	assert_eq!(any_event.downcast_mut::<MyEvent<i32>>(), None);
	any_event.downcast_mut::<MyEvent<String>>().unwrap().0 += " world";

	let any_event = any_event.downcast::<()>().unwrap_err();
	let inner = any_event.downcast::<MyEvent<String>>().unwrap();
	assert_eq!(inner.0, "hello world");
}
