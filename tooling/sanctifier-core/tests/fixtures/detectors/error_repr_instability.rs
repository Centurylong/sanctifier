#[contracterror]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    Unauthorized,
    InvalidInput,
}
