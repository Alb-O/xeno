use crate::nu::executor::NuExecError;

pub(crate) fn exec_error_message(error: &NuExecError) -> String {
	match error {
		NuExecError::Closed => "Nu executor has been shut down".to_string(),
		NuExecError::Transport(reason) => format!("Nu executor transport failure: {reason}"),
		NuExecError::Eval(msg) => msg.clone(),
	}
}
