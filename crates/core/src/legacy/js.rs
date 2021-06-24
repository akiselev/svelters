use rquickjs::ClassId;

use crate::inner::*;

// -------------------------------------------------------

#[allow(non_upper_case_globals)]
pub mod native_module {
    use crate::prelude::*;
    pub const n: i32 = 123;
    pub const s: &str = "abc";
    pub fn f(a: f64, b: f64) -> f64 {
        (a + b) * 0.5
    }
    pub struct Test<'js> {
        x: Persistent<Value<'js>>,
        y: String,
    }
    impl<'js> rquickjs::HasRefs for Test<'js> {
        fn mark_refs(&self, _marker: &rquickjs::RefsMarker) {}
    }
    impl<'js> Test<'js> {
        pub fn new(x: Persistent<Value<'js>>, y: String) -> Result<Test> {
            Ok(Test { x, y })
        }
        pub fn format(&'js self, ctx: Ctx<'js>, index: u64) -> Result<String> {
            let x = Persistent::restore(self.x.clone(), ctx)?
                .as_string()
                .unwrap()
                .to_string()?;
            Ok(format!("{}: {}", x.as_str(), index))
        }
        pub fn new_static(index: u64) -> Result<String> {
            Ok(format!("{}: ", index))
        }
        pub fn get(&'js self, ctx: Ctx<'js>) -> Result<Value<'js>> {
            Ok(Persistent::restore(self.x.clone(), ctx)?)
        }
        pub fn set(&'js mut self, ctx: Ctx<'js>, x: Value<'js>) {
            panic!("....")
        }
    }
}
pub struct NativeModule;
impl rquickjs::ModuleDef for NativeModule {
    fn load<'js>(
        _ctx: rquickjs::Ctx<'js>,
        exports: &rquickjs::Module<'js, rquickjs::Created>,
    ) -> rquickjs::Result<()> {
        exports.add("n")?;
        exports.add("s")?;
        exports.add("f")?;
        exports.add("Test")?;
        Ok(())
    }
    fn eval<'js>(
        _ctx: rquickjs::Ctx<'js>,
        exports: &rquickjs::Module<'js, rquickjs::Loaded<rquickjs::Native>>,
    ) -> rquickjs::Result<()> {
        exports.set("n", native_module::n)?;
        exports.set("s", native_module::s)?;
        exports.set("f", rquickjs::Func::new("f", native_module::f))?;

        rquickjs::Class::<native_module::Test>::register(_ctx)?;
        exports.set(
            "Test",
            rquickjs::Func::new(
                "Test",
                rquickjs::Class::<native_module::Test>::constructor(native_module::Test::new),
            ),
        )?;
        Ok(())
    }
}

impl<'this> rquickjs::ClassDef for native_module::Test<'this> {
    const CLASS_NAME: &'static str = "Test";
    unsafe fn class_id() -> &'static mut rquickjs::ClassId {
        static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new();
        &mut CLASS_ID
    }
    const HAS_PROTO: bool = true;
    fn init_proto<'js>(
        _ctx: rquickjs::Ctx<'js>,
        exports: &rquickjs::Object<'js>,
    ) -> rquickjs::Result<()> {
        exports.set(
            "format",
            rquickjs::Func::new("format", rquickjs::Method(native_module::Test::format)),
        )?;
        exports.prop(
            "x",
            rquickjs::Accessor::new(
                rquickjs::Method(native_module::Test::get),
                rquickjs::Method(native_module::Test::set),
            ),
        )?;
        Ok(())
    }
    const HAS_STATIC: bool = true;
    fn init_static<'js>(
        _ctx: rquickjs::Ctx<'js>,
        exports: &rquickjs::Object<'js>,
    ) -> rquickjs::Result<()> {
        exports.set(
            "new_static",
            rquickjs::Func::new("new_static", native_module::Test::new_static),
        )?;
        exports.set(
            "new_static",
            rquickjs::Func::new(
                "new_static",
                (
                    native_module::Test::new_static,
                    native_module::Test::new_static,
                ),
            ),
        )?;
        Ok(())
    }
    const HAS_REFS: bool = true;
    fn mark_refs(&self, marker: &rquickjs::RefsMarker) {
        rquickjs::HasRefs::mark_refs(self, marker);
    }
}
impl<'js> rquickjs::IntoJs<'js> for native_module::Test<'js> {
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        <native_module::Test as rquickjs::ClassDef>::into_js_obj(self, ctx)
    }
}
impl<'js> rquickjs::FromJs<'js> for &'js native_module::Test<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <native_module::Test as rquickjs::ClassDef>::from_js_ref(ctx, value)
    }
}
impl<'js> rquickjs::FromJs<'js> for &'js mut native_module::Test<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <native_module::Test as rquickjs::ClassDef>::from_js_mut(ctx, value)
    }
}
