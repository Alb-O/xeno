use super::*;

fn _assert_send<St: Send>(router: Router<St>) -> impl Send {
	router
}
