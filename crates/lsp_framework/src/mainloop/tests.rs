use super::*;

fn _main_loop_future_is_send<S>(f: MainLoop<S>, input: impl AsyncBufRead + Send + Unpin, output: impl AsyncWrite + Send + Unpin) -> impl Send
where
	S: LspService<Response = JsonValue> + Send + 'static,
	S::Future: Send + 'static,
	ResponseError: From<S::Error>,
{
	f.run(input, output)
}
