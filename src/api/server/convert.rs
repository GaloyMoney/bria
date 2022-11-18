use crate::error::BriaError;

impl From<BriaError> for tonic::Status {
    fn from(err: BriaError) -> Self {
        tonic::Status::new(tonic::Code::Unknown, format!("{}", err))
    }
}
