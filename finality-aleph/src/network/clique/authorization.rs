use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};

#[derive(Debug, PartialEq, Eq)]
pub enum AuthorizatorError {
    MissingService,
    ServiceDisappeared,
}

/// Allows one to authorize incoming public-keys.
#[async_trait::async_trait]
pub trait Authorization<PK> {
    async fn is_authorized(&self, value: PK) -> Result<bool, AuthorizatorError>;
}

struct AuthorizationHandler<PK> {
    identifier: PK,
    result_sender: oneshot::Sender<bool>,
}

impl<PK> AuthorizationHandler<PK> {
    fn new(result: PK) -> (Self, oneshot::Receiver<bool>) {
        let (auth_sender, auth_receiver) = oneshot::channel();
        (
            Self {
                identifier: result,
                result_sender: auth_sender,
            },
            auth_receiver,
        )
    }

    pub fn handle_authorization(
        self,
        handler: impl FnOnce(PK) -> bool,
    ) -> Result<(), AuthorizatorError> {
        let auth_result = handler(self.identifier);
        self.result_sender
            .send(auth_result)
            .map_err(|_| AuthorizatorError::MissingService)
    }
}

/// Used for validation of authorization requests. One should call [handle_authorization](Self::handle_authorization) and
/// provide a callback responsible for authorization. Each such call should be matched with call to
/// [Authorizator::is_authorized](Authorizator::is_authorized).
pub struct AuthorizationRequestHandler<PK> {
    receiver: mpsc::UnboundedReceiver<AuthorizationHandler<PK>>,
}

impl<PK> AuthorizationRequestHandler<PK> {
    fn new(receiver: mpsc::UnboundedReceiver<AuthorizationHandler<PK>>) -> Self {
        Self { receiver }
    }

    pub async fn handle_authorization<F: FnOnce(PK) -> bool>(
        &mut self,
        handler: F,
    ) -> Result<(), AuthorizatorError> {
        let next = self
            .receiver
            .next()
            .await
            .ok_or(AuthorizatorError::MissingService)?;

        next.handle_authorization(handler)
    }
}

#[derive(Clone)]
pub struct Authorizator<PK> {
    sender: mpsc::UnboundedSender<AuthorizationHandler<PK>>,
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
        let (handler, receiver) = AuthorizationHandler::new(value);
        self.sender
            .unbounded_send(handler)
            .map_err(|_| AuthorizatorError::MissingService)?;
        receiver
            .await
            .map_err(|_| AuthorizatorError::ServiceDisappeared)
    }
}

#[cfg(test)]
mod tests {
    use futures::join;

    use crate::network::clique::{
        authorization::{Authorization, Authorizator, AuthorizatorError},
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
            request_handler.handle_authorization(|_| true),
        );

        assert_eq!(
            authorizator_result.expect("Authorizator should return Ok."),
            true
        );
        assert_eq!(
            request_handler_result.expect("Request handler should return Ok."),
            ()
        );

        let (authorizator_result, request_handler_result) = join!(
            authorizator.is_authorized(public_key),
            request_handler.handle_authorization(|_| false),
        );

        assert_eq!(
            authorizator_result.expect("Authorizator should return Ok."),
            false
        );
        assert_eq!(
            request_handler_result.expect("Request handler should return Ok."),
            ()
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
        let result = request_handler.handle_authorization(|_| true).await;

        assert_eq!(result, Err(AuthorizatorError::MissingService))
    }
}
