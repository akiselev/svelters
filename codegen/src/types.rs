use select::document::Document;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassDef {
    pub name: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QClass {
    pub class: ClassDef,
    pub inherits: Vec<ClassDef>,
    pub signals: Vec<QFn>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QFn {
    pub name: String,
    pub args: Vec<QArg>,
    pub return_type: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QArg {
    pub is_const: bool,
    pub is_ptr: bool,
    pub is_ref: bool,
    pub has_default: bool,
    pub _type: String,
}
