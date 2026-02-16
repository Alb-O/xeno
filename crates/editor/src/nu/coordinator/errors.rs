use crate::nu::executor::NuExecError;

pub(crate) fn exec_error_message(error: &NuExecError) -> String {
	match error {
		NuExecError::Shutdown { .. } => "Nu executor thread has shut down".to_string(),
		NuExecError::ReplyDropped => "Nu executor worker died during evaluation".to_string(),
		NuExecError::Eval(msg) => msg.clone(),
	}
}
