
//obj.rs(55, 9): Error originated from macro call here
impl Implements<init_static::InitStatic<'_, '_, FW, CLS>> for JsClassLoader<FW, CLS> {}

//traits.rs(131, 8): Error originated from macro call here
impl Implements<has_member::HasMember<'_, FW, CLS>> for JsClassLoader<FW, CLS> {}

//traits.rs(143, 8): Error originated from macro call here
impl Implements<inherits_member::InheritsMember<'_, FW, CLS>> for JsClassLoader<FW, CLS> {}

//inherits_member.rs(44, 12): Error originated from macro call here
impl Implements<has_member::HasMember<'_, FW, SUPER>> for JsClassLoader<FW, SUPER> {}

//inherits_member.rs(49, 25): Error originated from macro call here
impl Implements<inherits_member::InheritsMember<'_, FW, SUPER>> for JsClassLoader<FW, SUPER> {}

//init_proto.rs(135, 9): Error originated from macro call here
impl Implements<super_proto::InitSuperProto<'_, '_, FW, CLS, SUPER>> for JsClassLoader<FW, SUPER> {}

//init_static.rs(53, 9): Error originated from macro call here
impl Implements<super_static::InitSuperStatic<'_, '_, FW, CLS, SUPER>> for JsClassLoader<FW, SUPER> {}

//nested_super_proto.rs(33, 12): Error originated from macro call here
impl Implements<has_member::HasMember<'_, FW, ROOT>> for JsClassLoader<FW, ROOT> {}

//nested_super_proto.rs(37, 12): Error originated from macro call here
impl Implements<inherits_member::InheritsMember<'_, FW, ROOT>> for JsClassLoader<FW, ROOT> {}

//nested_super_proto.rs(167, 9): Error originated from macro call here
impl Implements<nested_super_proto::NestedSuperProto<'_, '_, FW, ROOT, INTER, SUPER>> for JsClassLoader<FW, SUPER> {}

//super_proto.rs(150, 9): Error originated from macro call here
impl Implements<nested_super_proto::NestedSuperProto<'_, '_, FW, CLS, SUPER, NESTED>> for JsClassLoader<FW, NESTED> {}

//init_caster.rs(40, 33): Error originated from macro call here
impl Implements<InitCaster<'_, FW, CLS>> for JsClassLoader<FW, CLS> {}

//init_caster.rs(92, 9): Error originated from macro call here
impl Implements<init_super::InitSuperCaster<'_, FW, CLS, SUPER>> for JsClassLoader<FW, CLS> {}

//init_nested_super.rs(43, 9): Error originated from macro call here
impl Implements<init_nested_super::NestedSuperCaster<'_, FW, ROOT, INTER, SUPER>> for JsClassLoader<FW, SUPER> {}

//init_super.rs(40, 9): Error originated from macro call here
impl Implements<init_nested_super::NestedSuperCaster<'_, FW, CLS, SUPER, NESTED>> for JsClassLoader<FW, NESTED> {}

//loader.rs(81, 9): Error originated from macro call here
impl Implements<constructor_eval::ConstructorEval<'_, '_, FW, CLS>> for JsClassLoader<FW, CLS> {}

//loader.rs(93, 9): Error originated from macro call here
impl Implements<constructor_load::ConstructorLoad<'_, '_, FW, CLS>> for JsClassLoader<FW, CLS> {}