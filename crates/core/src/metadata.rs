use crate::inner::*;

pub struct Metadata {
    pub(in crate) obj_id: ClassId,
}

impl Metadata {
    pub const fn new() -> Metadata {
        Metadata {
            obj_id: ClassId::new(),
        }
    }
}
