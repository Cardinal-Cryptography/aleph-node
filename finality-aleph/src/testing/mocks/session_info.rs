use std::sync::{Arc, Mutex};

use aleph_primitives::BlockNumber;
use futures::future::pending;

use super::TBlockIdentifier;
use crate::{
    justification::{AlephJustification, SessionInfo, SessionInfoProvider, Verifier},
    session::SessionBoundaryInfo as SessionBoundInfo,
    testing::mocks::AcceptancePolicy,
    SessionPeriod,
};

pub struct VerifierWrapper {
    acceptance_policy: Arc<Mutex<AcceptancePolicy>>,
}

impl Verifier<TBlockIdentifier> for VerifierWrapper {
    fn verify(&self, _justification: &AlephJustification, _block_id: &TBlockIdentifier) -> bool {
        self.acceptance_policy.lock().unwrap().accepts()
    }
}

pub struct SessionInfoProviderImpl {
    session_info: SessionBoundInfo,
    acceptance_policy: Arc<Mutex<AcceptancePolicy>>,
}

impl SessionInfoProviderImpl {
    pub fn new(session_period: SessionPeriod, acceptance_policy: AcceptancePolicy) -> Self {
        Self {
            session_info: SessionBoundInfo::new(session_period),
            acceptance_policy: Arc::new(Mutex::new(acceptance_policy)),
        }
    }
}

#[async_trait::async_trait]
impl SessionInfoProvider<TBlockIdentifier, VerifierWrapper> for SessionInfoProviderImpl {
    type Error = &'static str;

    async fn for_block_num(
        &self,
        number: BlockNumber,
    ) -> Result<SessionInfo<TBlockIdentifier, VerifierWrapper>, Self::Error> {
        let current_session = self.session_info.session_id_from_block_num(number);
        let acceptance_policy = (*self.acceptance_policy.lock().unwrap()).clone();
        if let AcceptancePolicy::Unavailable = acceptance_policy {
            let res =
                pending::<Result<SessionInfo<TBlockIdentifier, VerifierWrapper>, Self::Error>>()
                    .await;
            res
        } else {
            Ok(SessionInfo::new(
                self.session_info.last_block_of_session(current_session),
                VerifierWrapper {
                    acceptance_policy: self.acceptance_policy.clone(),
                },
            ))
        }
    }
}
