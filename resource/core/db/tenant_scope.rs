use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct TenantId(pub Uuid);

pub struct TenantTx {
    // populated in Task 3
    _placeholder: (),
}
