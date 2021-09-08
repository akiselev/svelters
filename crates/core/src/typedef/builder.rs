use rquickjs::Function;

use crate::inner::*;

pub struct TypeBuilder<CLS, PHASE> {
    phase: PHASE,
    pub(in crate) marker: PhantomData<CLS>,
}

impl<CLS, PHASE> TypeBuilder<CLS, PHASE>
where
    CLS: Type,
{
    #[inline(always)]
    pub fn new(phase: PHASE) -> Self {
        Self {
            phase,
            marker: PhantomData,
        }
    }

    #[inline(always)]
    pub fn class(self) -> Result<ClassBuilder<CLS, PHASE>>
    where
        JsClassLoader<CLS>: Implements<PHASE>,
        ClassBuilder<CLS, PHASE>: Implements<PHASE>,
    {
        let phase = JsClassLoader::<CLS>::new().build_phase(self.phase)?;
        Ok(ClassBuilder::new(phase))
    }
}

pub struct ClassBuilder<CLS, PHASE> {
    phase: PHASE,
    pub(in crate) marker: PhantomData<CLS>,
}

impl<CLS, PHASE> ClassBuilder<CLS, PHASE> {
    pub fn new(phase: PHASE) -> Self {
        Self {
            phase,
            marker: PhantomData,
        }
    }

    pub fn build(self) -> Result<Self>
    where
        Self: TypeDef<PHASE>,
    {
        Ok(self)
    }

    pub fn method<RET, FN, ARGS>(self, name: &'static str, func: FN) -> Result<Self>
    where
        CLS: 'static + Type,
        ARGS: for<'js> FromImpl<'js, CLS>,
        RET: 'static + for<'js> ToResult<'js>,
        FN: 'static + for<'x> Fn<ARGS, Output = RET>,
        members::Member<CLS, members::AsMethod<FN>>: Implements<PHASE>,
    {
        let phase =
            members::Member::new(name, members::AsMethod::new(func)).build_phase(self.phase)?;
        Ok(ClassBuilder::new(phase))
    }

    #[inline(always)]
    pub fn prop<PROP, GET, SET>(self, name: &'static str, getter: GET, setter: SET) -> Result<Self>
    where
        CLS: Cast<CLS>,
        PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
        GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
        SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
        members::Property<CLS, PROP, GET, SET>: Implements<PHASE>,
    {
        let phase = members::Property::<CLS, PROP, GET, SET>::new(name, getter, setter)
            .build_phase(self.phase)?;
        Ok(ClassBuilder::new(phase))
    }

    #[inline(always)]
    pub fn readonly<PROP, GET>(self, name: &'static str, getter: GET) -> Result<Self>
    where
        CLS: Cast<CLS>,
        PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
        GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
        members::ReadOnly<CLS, PROP, GET>: Implements<PHASE>,
    {
        let phase =
            members::ReadOnly::<CLS, PROP, GET>::new(name, getter).build_phase(self.phase)?;
        Ok(ClassBuilder::new(phase))
    }

    #[inline(always)]
    pub fn superclass<SUPER>(self) -> Result<Self>
    where
        SUPER: Type,
        CLS: Cast<SUPER>,
        members::Superclass<SUPER>: Implements<PHASE>,
    {
        let phase = members::Superclass::<SUPER>::new().build_phase(self.phase)?;
        Ok(ClassBuilder::new(phase))
    }

    #[inline(always)]
    pub fn as_element<FUNC>(self, name: &'static str, func: FUNC) -> Result<Self>
    where
        FUNC: 'static + Fn() -> Result<CLS>,
        members::Element<CLS, FUNC>: Implements<PHASE>,
    {
        let phase = members::Element::<CLS, FUNC>::new(name, func).build_phase(self.phase)?;
        Ok(ClassBuilder::new(phase))
    }

    #[inline(always)]
    pub fn with_slot<SLOT>(self, slot: SLOT) -> Result<Self>
    where
        SLOT: crate::traits::INativeSlot,
        members::NativeSlot<CLS, SLOT>: Implements<PHASE>,
    {
        let phase = members::NativeSlot::<CLS, SLOT>::new(slot).build_phase(self.phase)?;
        Ok(ClassBuilder::new(phase))
    }

    #[inline(always)]
    pub fn with_event<FUNC>(self, name: &'static str, func: FUNC) -> Result<Self>
    where
        members::Event<CLS, FUNC>: Implements<PHASE>,
        FUNC: 'static + for<'js> Fn(Element<'js>, Function<'js>) -> Result<()>,
    {
        let phase = members::Event::<CLS, FUNC>::new(name, func).build_phase(self.phase)?;
        Ok(ClassBuilder::new(phase))
    }
}

macro_rules! tup {
    ($($static_fn:ident $method:ident $constructor:ident $($arg:ident)*,)*) => {
        $(
            #[inline(always)]
            pub fn $constructor<FUNC, $($arg,)*>(self, func: FUNC) -> Result<Self>
            where
                CLS: 'static + Type,
                FUNC: 'static + Fn($($arg,)*) -> Result<CLS>,
                members::Constructor<CLS, ($($arg,)*), FUNC>: Implements<PHASE>,
                $($arg: for<'x> FromJs<'x>,)*
            {
                let phase = members::Constructor::new(func).build_phase(self.phase)?;
                Ok(ClassBuilder::new(phase))
            }

            #[inline(always)]
            pub fn $method<'js, THIS, RET, FN, $($arg,)*>(self, name: &'static str, func: FN) -> Result<Self>
            where
                CLS: 'static + Type,
                CLS: Cast<THIS>,
                THIS: 'static,
                RET: 'static +  for<'x> IntoJs<'x>,
                FN: 'static + for<'x> Fn(&'x THIS, $($arg,)*) -> Result<RET>,
                members::Method<CLS, (THIS, $($arg,)*), RET, FN>: Implements<PHASE>,
                $($arg: FromJs<'js>,)*
            {
                let phase = members::Method::new(name, func).build_phase(self.phase)?;
                Ok(ClassBuilder::new(phase))
            }

            #[inline(always)]
            pub fn $static_fn<RET, FN, $($arg,)*>(self, name: &'static str, func: FN) -> Result<Self>
            where
                FN: 'static + Fn($($arg,)*) -> Result<RET>,
                CLS: 'static + Type,
                RET: 'static + for<'js> IntoJs<'js>,
                members::StaticFn<CLS, ($($arg,)*), RET, FN>: Implements<PHASE>,
                $($arg: for<'x> FromJs<'x>,)*
            {
                let phase = members::StaticFn::new(name, func).build_phase(self.phase)?;
                Ok(ClassBuilder::new(phase))
            }

        )*
    };
    (@one $($t:tt)*) => { 1 };
    (@idx A) => { 0 };
    (@idx B) => { 1 };
    (@idx C) => { 2 };
    (@idx D) => { 3 };
    (@idx E) => { 4 };
    (@idx F) => { 5 };
    (@idx G) => { 6 };
    (@idx H) => { 7 };
    (@idx I) => { 8 };
    (@idx J) => { 9 };
    (@idx K) => { 10 };
    (@idx L) => { 11 };
    (@idx M) => { 12 };
    (@idx N) => { 13 };
    (@idx O) => { 14 };
    (@idx P) => { 15 };
}

impl<CLS, PHASE> ClassBuilder<CLS, PHASE> {
    tup! {
        static_fn0 method0 constructor0 ,
        static_fn1 method1 constructor1 A,
        static_fn2 method2 constructor2 A B,
        static_fn3 method3 constructor3 A B C,
        static_fn4 method4 constructor4 A B C D,
        static_fn5 method5 constructor5 A B C D E,
        static_fn6 method6 constructor6 A B C D E F,
        static_fn7 method7 constructor7 A B C D E F G,
        static_fn8 method8 constructor8 A B C D E F G H,
        static_fn9 method9 constructor9 A B C D E F G H I,
        static_fn10 method10 constructor10 A B C D E F G H I J,
        static_fn11 method11 constructor11 A B C D E F G H I J K,
        static_fn12 method12 constructor12 A B C D E F G H I J K L,
        static_fn13 method13 constructor13 A B C D E F G H I J K L M,
        static_fn14 method14 constructor14 A B C D E F G H I J K L M N,
        static_fn15 method15 constructor15 A B C D E F G H I J K L M N O,
        static_fn16 method16 constructor16 A B C D E F G H I J K L M N O P,
    }
}

impl<CLS, PHASE> TypeDef<PHASE> for ClassBuilder<CLS, PHASE> {}
