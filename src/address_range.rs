#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AddressRangeType {
    /// May have contents
    Contents,
    /// Must be uninitialized
    NoContents,
    /// will be ignored
    Ignore,
}

#[derive(Copy, Clone, Debug)]
pub struct AddressRange {
    pub typ: AddressRangeType,
    pub to: u32,
    pub from: u32,
}

impl AddressRange {
    pub const fn new(from: u32, to: u32, typ: AddressRangeType) -> Self {
        Self { typ, to, from }
    }
}
