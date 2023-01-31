use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};

#[derive(Debug, PartialEq, Eq)]
pub enum AuthorizatorError {
    MissingService,
    ServiceDisappeared,
}

pub enum AuthorizationResult {
    Authorized,
    NotAuthorized,
}

/// Allows one to authorize incoming public-keys.
#[async_trait::async_trait]
pub trait Authorization<PK> {
    async fn is_authorized(&self, value: PK) -> Result<bool, AuthorizatorError>;
}

/// Used for validation of authorization requests. One should call [handle_authorization](Self::handle_authorization) and
/// provide a callback responsible for authorization. Each such call should be matched with call to
/// [Authorizator::is_authorized](Authorizator::is_authorized).
pub struct AuthorizationRequestHandler<PK> {
    receiver: mpsc::UnboundedReceiver<(PK, oneshot::Sender<AuthorizationResult>)>,
}

impl<PK> AuthorizationRequestHandler<PK> {
    fn new(receiver: mpsc::UnboundedReceiver<(PK, oneshot::Sender<AuthorizationResult>)>) -> Self {
        Self { receiver }
    }

    pub async fn handle_authorization<F: FnOnce(PK) -> AuthorizationResult>(
        &mut self,
        handler: F,
    ) -> Result<(), AuthorizatorError> {
        let (identifier, result_sender) = self
            .receiver
            .next()
            .await
            .ok_or(AuthorizatorError::MissingService)?;

        let auth_result = handler(identifier);
        result_sender
            .send(auth_result)
            .map_err(|_| AuthorizatorError::MissingService)?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct Authorizator<PK> {
    sender: mpsc::UnboundedSender<(PK, oneshot::Sender<AuthorizationResult>)>,
}

/// `Authorizator` is responsible for authorization of public-keys for the validator-network component. Each call to
/// [is_authorized](Authorizator::is_authorized) should be followed by a call of
/// [handle_authorization](AuthorizationHandler::handle_authorization).
impl<PK> Authorizator<PK> {
    pub fn new() -> (Self, AuthorizationRequestHandler<PK>) {
        let (sender, receiver) = mpsc::unbounded();
        (Self { sender }, AuthorizationRequestHandler::new(receiver))
    }
}

#[async_trait::async_trait]
impl<PK: Send> Authorization<PK> for Authorizator<PK> {
    async fn is_authorized(&self, value: PK) -> Result<bool, AuthorizatorError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .unbounded_send((value, sender))
            .map_err(|_| AuthorizatorError::MissingService)?;
        receiver
            .await
            .map_err(|_| AuthorizatorError::ServiceDisappeared)
            .map(|auth_result| match auth_result {
                AuthorizationResult::Authorized => true,
                AuthorizationResult::NotAuthorized => false,
            })
    }
}

#[cfg(test)]
mod tests {
    use futures::join;

    use crate::network::clique::{
        authorization::{Authorization, AuthorizationResult, Authorizator, AuthorizatorError},
        mock::{key, MockSecretKey},
        SecretKey,
    };

    #[tokio::test]
    async fn authorization_sanity_check() {
        let (authorizator, mut request_handler) =
            Authorizator::<<MockSecretKey as SecretKey>::PublicKey>::new();
        let public_key = key().0;
        let (authorizator_result, request_handler_result) = join!(
            authorizator.is_authorized(public_key.clone()),
            request_handler.handle_authorization(|_| AuthorizationResult::Authorized),
        );

        assert!(authorizator_result.expect("Authorizator should return Ok."));
        assert!(
            request_handler_result.is_ok(),
            "Request handler should return Ok."
        );

        let (authorizator_result, request_handler_result) = join!(
            authorizator.is_authorized(public_key),
            request_handler.handle_authorization(|_| AuthorizationResult::NotAuthorized),
        );

        assert!(!authorizator_result.expect("Authorizator should return Ok."));
        assert!(
            request_handler_result.is_ok(),
            "Request handler should return Ok."
        );
    }

    #[tokio::test]
    async fn authorizator_returns_error_when_handler_is_dropped() {
        let (authorizator, request_handler) =
            Authorizator::<<MockSecretKey as SecretKey>::PublicKey>::new();
        let public_key = key().0;
        drop(request_handler);
        let result = authorizator.is_authorized(public_key.clone()).await;

        assert_eq!(result, Err(AuthorizatorError::MissingService))
    }

    #[tokio::test]
    async fn authorizator_returns_error_when_handler_disappeared() {
        let (authorizator, mut request_handler) =
            Authorizator::<<MockSecretKey as SecretKey>::PublicKey>::new();
        let public_key = key().0;
        let (authorizator_result, _) = join!(
            authorizator.is_authorized(public_key.clone()),
            tokio::spawn(async move {
                request_handler
                    .handle_authorization(|_| panic!("handler bye bye"))
                    .await
            }),
        );

        assert_eq!(
            authorizator_result,
            Err(AuthorizatorError::ServiceDisappeared)
        )
    }

    #[tokio::test]
    async fn authorization_request_handler_returns_error_when_all_authorizators_are_missing() {
        let (authorizator, mut request_handler) =
            Authorizator::<<MockSecretKey as SecretKey>::PublicKey>::new();
        drop(authorizator);
        let result = request_handler
            .handle_authorization(|_| AuthorizationResult::Authorized)
            .await;

        assert_eq!(result, Err(AuthorizatorError::MissingService))
    }
}
