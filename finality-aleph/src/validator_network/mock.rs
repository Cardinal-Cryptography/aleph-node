
pub struct MockPrelims<D> {
    pub id_incoming: MockPublicKey,
    pub pen_incoming: MockSecretKey,
    pub id_outgoing: MockPublicKey,
    pub pen_outgoing: MockSecretKey,
    pub incoming_handle: Pin<Box<dyn Future<Output = Result<(), ProtocolError<MockPublicKey>>>>>,
    pub outgoing_handle: Pin<Box<dyn Future<Output = Result<(), ProtocolError<MockPublicKey>>>>>,
    pub data_from_incoming: UnboundedReceiver<D>,
    pub data_from_outgoing: Option<UnboundedReceiver<D>>,
    pub result_from_incoming: UnboundedReceiver<ResultForService<MockPublicKey, D>>,
    pub result_from_outgoing: UnboundedReceiver<ResultForService<MockPublicKey, D>>,
}
