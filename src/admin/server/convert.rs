use crate::admin::AdminApiError;

impl From<AdminApiError> for tonic::Status {
    fn from(err: AdminApiError) -> Self {
        tonic::Status::new(tonic::Code::Unknown, format!("{err}"))
    }
}
