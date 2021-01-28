pub struct PageFragment {
    file_offset: u32,
    page_offset: u32,
    bytes: u32,
}

impl PageFragment {
    fn new(file_offset: u32, page_offset: u32, bytes: u32) -> Self {
        Self {
            file_offset,
            page_offset,
            bytes,
        }
    }
}
