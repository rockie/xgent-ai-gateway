pub mod xgent {
    pub mod gateway {
        pub mod v1 {
            tonic::include_proto!("xgent.gateway.v1");
        }
    }
}

pub use xgent::gateway::v1::*;
