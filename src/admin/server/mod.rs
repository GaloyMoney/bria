#[allow(clippy::all)]
pub mod proto {
    tonic::include_proto!("services.admin.v1");
}

use proto::{admin_service_server::AdminService, *};

pub struct Admin {}
