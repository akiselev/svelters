use qt_core::{
    q_meta_method::{Access, MethodType},
    q_meta_type::TypeFlag,
};

use crate::common::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct TypeId(pub(crate) c_int);

impl From<TypeId> for c_int {
    fn from(val: TypeId) -> Self {
        val.0
    }
}

pub struct TypeInfo {
    id: TypeId,
    meta: CppBox<QMetaObject>,
}

impl Clone for TypeInfo {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            meta: unsafe { QMetaObject::new_copy(self.meta.as_ref()) },
        }
    }
}

impl TypeInfo {
    pub fn from_type(id: c_int) -> Result<Self> {
        let meta = unsafe { QMetaType::meta_object_for_type(id).as_ref() }
            .ok_or_else(|| exception!("QMetaObject is null"))?;
        Ok(Self {
            meta: unsafe { QMetaObject::new_copy(meta) },
            id: TypeId(id),
        })
    }

    pub fn from_name<T>(val: &str) -> Result<Self> {
        unsafe {
            let name = CString::new(val).map_err(|err| exception!("CString error: {}", err))?;
            let id = QMetaType::type_char(name.as_ptr());
            if id == QType::UnknownType.to_int() {
                return Err(exception!("Unknown type: {}", val));
            }
            Self::from_type(id)
        }
    }

    pub fn new<T>(val: T) -> Result<Self>
    where
        Ptr<QObject>: CastFrom<T>,
    {
        unsafe {
            let obj: Ptr<QObject> = val.cast_into();
            let meta = obj.meta_object();
            if meta.is_null() {
                return Err(exception!(
                    "Unknown type: {:?}",
                    CStr::from_ptr(meta.class_name())
                ));
            }
            let id = QMetaType::type_char(meta.class_name());
            if id == QType::UnknownType.to_int() {
                return Err(exception!(
                    "Unknown type: {:?}",
                    CStr::from_ptr(meta.class_name())
                ));
            }
            Ok(Self {
                meta: unsafe { QMetaObject::new_copy(meta.as_ref().expect("QMetaObject is null")) },
                id: TypeId(id),
            })
        }
    }

    pub fn id(&self) -> TypeId {
        self.id
    }

    pub fn name(&self) -> String {
        unsafe {
            let array = self.meta.class_name();
            let name = CStr::from_ptr(array);
            name.to_str()
                .expect("UTF8 Error in Property Name")
                .to_owned()
        }
    }

    pub fn is_object(&self) -> bool {
        unsafe { self.meta.inherits(QObject::static_meta_object()) }
    }

    unsafe fn meta_type(&self) -> CppBox<QMetaType> {
        QMetaType::new(self.id.0)
    }

    pub fn properties(self) -> PropertyIter {
        unsafe {
            PropertyIter {
                meta: self,
                index: 0,
                count: self.meta.property_count(),
            }
        }
    }

    pub fn methods(self) -> MethodIter {
        unsafe {
            MethodIter {
                meta: self,
                index: 0,
                count: self.meta.method_count(),
                ty: MethodType::Method,
            }
        }
    }

    pub fn signals(self) -> MethodIter {
        unsafe {
            MethodIter {
                meta: self,
                index: 0,
                count: self.meta.method_count(),
                ty: MethodType::Signal,
            }
        }
    }

    pub fn slots(self) -> MethodIter {
        unsafe {
            MethodIter {
                meta: self,
                index: 0,
                count: self.meta.method_count(),
                ty: MethodType::Slot,
            }
        }
    }

    pub fn constructors(self) -> ConstructorIter {
        unsafe {
            ConstructorIter {
                meta: self,
                index: 0,
                count: self.meta.constructor_count(),
            }
        }
    }

    pub fn is_object_ptr(&self) -> bool {
        unsafe {
            self.meta_type()
                .flags()
                .test_flag(TypeFlag::PointerToQObject)
        }
    }
}

pub struct PropertyInfo(CppBox<QMetaProperty>);

impl PropertyInfo {
    pub fn name(&self) -> String {
        unsafe {
            let array = self.0.name();
            let name = CStr::from_ptr(array);
            name.to_str()
                .expect("UTF8 Error in Property Name")
                .to_owned()
        }
    }
}

impl std::ops::Deref for PropertyInfo {
    type Target = CppBox<QMetaProperty>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
pub struct MethodInfo(CppBox<QMetaMethod>);

impl MethodInfo {
    pub fn name(&self) -> String {
        unsafe {
            let array = self.0.name();
            let name = CStr::from_ptr(array.to_char());
            name.to_str().expect("UTF8 Error in Method Name").to_owned()
        }
    }

    pub fn num_args(&self) -> c_int {
        unsafe { self.0.parameter_count() }
    }

    pub fn args(&self) -> ParameterIter {
        unsafe {
            ParameterIter {
                meta: self,
                index: 0,
                count: self.0.parameter_count(),
            }
        }
    }

    pub fn kind(&self) -> MethodKind {
        unsafe {
            let ty = self.method_type();
            match ty {
                MethodType::Method => MethodKind::Method,
                MethodType::Signal => MethodKind::Signal,
                MethodType::Slot => MethodKind::Slot,
                MethodType::Constructor => MethodKind::Constructor,
            }
        }
    }

    pub fn return_type(&self) -> Result<TypeInfo> {
        unsafe { TypeInfo::from_type(self.0.return_type()) }
    }
}

impl std::ops::Deref for MethodInfo {
    type Target = CppBox<QMetaMethod>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct PropertyIter {
    meta: TypeInfo,
    index: i32,
    count: i32,
}

impl Iterator for PropertyIter {
    type Item = PropertyInfo;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.count {
            let meta = unsafe { self.meta.meta.property(self.index) };
            self.index += 1;
            Some(PropertyInfo(meta))
        } else {
            None
        }
    }
}

pub struct MethodIter {
    meta: TypeInfo,
    index: i32,
    count: i32,
    ty: MethodType,
}

impl Iterator for MethodIter {
    type Item = MethodInfo;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            while self.index < self.count {
                let meta = unsafe { self.meta.meta.method(self.index) };
                self.index += 1;
                if meta.is_valid()
                    && meta.access() == Access::Public
                    && meta.method_type() == self.ty
                {
                    return Some(MethodInfo(meta));
                }
            }
        }
        None
    }
}

pub struct ConstructorIter {
    meta: TypeInfo,
    index: i32,
    count: i32,
}

impl Iterator for ConstructorIter {
    type Item = MethodInfo;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.count {
            let meta = unsafe { self.meta.meta.constructor(self.index) };
            self.index += 1;
            Some(MethodInfo(meta))
        } else {
            None
        }
    }
}

pub struct ParameterIter<'a> {
    meta: &'a MethodInfo,
    index: i32,
    count: i32,
}

impl<'a> Iterator for ParameterIter<'a> {
    type Item = ParameterInfo<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.count {
            let meta = ParameterInfo {
                meta: self.meta,
                index: self.index,
            };
            self.index += 1;
            Some(meta)
        } else {
            None
        }
    }
}

pub struct ParameterInfo<'a> {
    meta: &'a MethodInfo,
    index: i32,
}

impl<'a> ParameterInfo<'a> {
    pub fn typeinfo(&self) -> Result<TypeInfo> {
        let id = unsafe { self.meta.parameter_type(self.index) };
        TypeInfo::from_type(id)
    }
}

pub enum MethodKind {
    Method,
    Signal,
    Slot,
    Constructor,
}
