export interface ExternalCrate {
    name: string;
    html_root_url: string | null;
}

export interface ItemSummary {
    crate_id: number;
    path: string[];
    kind: ItemKind;
}

export type ItemBase = {
    id: Id;
    crate_id: number;
    name: string | null;
    span: Span | null;
    visibility: Visibility;
    docs: string | null;
    links: { [key: string]: Id };
    attrs: string[];
    deprecation: Deprecation | null;
};

export type Item = ItemEnum & ItemBase;

export interface Span {
    filename: string;
    begin: [number, number];
    end: [number, number];
}

export interface Deprecation {
    since: string | null;
    note: string | null;
}

export interface Constant {
    type: Type;
    expr: string;
    value: string | null;
    is_literal: boolean;
}

export interface TypeBinding {
    name: string;
    binding: TypeBindingKind;
}

export type Id = string;

export interface Module {
    is_crate: boolean;
    items: Id[];
}

export interface Union {
    generics: Generics;
    fields_stripped: boolean;
    fields: Id[];
    impls: Id[];
}

export interface Struct {
    struct_type: StructType;
    generics: Generics;
    fields_stripped: boolean;
    fields: Id[];
    impls: Id[];
}

export interface Enum {
    generics: Generics;
    variants_stripped: boolean;
    variants: Id[];
    impls: Id[];
}

export interface Function {
    decl: FnDecl;
    generics: Generics;
    header: Qualifiers[];
    abi: string;
}

export interface Method {
    decl: FnDecl;
    generics: Generics;
    header: Qualifiers[];
    abi: string;
    has_body: boolean;
}

export interface Generics {
    params: GenericParamDef[];
    where_predicates: WherePredicate[];
}

export interface GenericParamDef {
    name: string;
    kind: GenericParamDefKind;
}

export interface FunctionPointer {
    decl: FnDecl;
    generic_params: GenericParamDef[];
    header: Qualifiers[];
    abi: string;
}

export interface FnDecl {
    inputs: [string, Type][];
    output: Type | null;
    c_variadic: boolean;
}

export interface Trait {
    is_auto: boolean;
    is_unsafe: boolean;
    items: Id[];
    generics: Generics;
    bounds: GenericBound[];
    implementors: Id[];
}

export interface TraitAlias {
    generics: Generics;
    params: GenericBound[];
}

export interface Impl {
    is_unsafe: boolean;
    generics: Generics;
    provided_trait_methods: string[];
    trait: Type | null;
    for: Type;
    items: Id[];
    negative: boolean;
    synthetic: boolean;
    blanket_impl: Type | null;
}

export interface Import {
    source: string;
    name: string;
    id: Id | null;
    glob: boolean;
}

export interface ProcMacro {
    kind: MacroKind;
    helpers: string[];
}

export interface Typedef {
    type: Type;
    generics: Generics;
}

export interface OpaqueTy {
    bounds: GenericBound[];
    generics: Generics;
}

export interface Static {
    type: Type;
    mutable: boolean;
    expr: string;
}

export type Visibility =
    | "public"
    | "default"
    | "crate"
    | {
          parent: Id;
          path: string;
      };

export type GenericArgs =
    | {
          args: GenericArg[];
          bindings: TypeBinding[];
      }
    | {
          inputs: Type[];
          output: Type | null;
      };

export type GenericArg = string | Type | Constant;

export type TypeBindingKind = Type | GenericBound[];

export type ItemKind =
    | "module"
    | "extern_crate"
    | "import"
    | "struct"
    | "struct_field"
    | "union"
    | "enum"
    | "variant"
    | "function"
    | "typedef"
    | "opaque_ty"
    | "constant"
    | "trait"
    | "trait_alias"
    | "method"
    | "impl"
    | "static"
    | "foreign_type"
    | "macro"
    | "proc_attribute"
    | "proc_derive"
    | "assoc_const"
    | "assoc_type"
    | "primitive"
    | "keyword";

export type ItemEnum =
    | { kind: "module"; inner: Module }
    | {
          kind: "extern_crate";
          inner: {
              name: string;
              rename: string | null;
          };
      }
    | { kind: "import"; inner: Import }
    | { kind: "union"; inner: Union }
    | { kind: "struct"; inner: Struct }
    | { kind: "struct_field"; inner: Type }
    | { kind: "enum"; inner: Enum }
    | { kind: "variant"; inner: Variant }
    | { kind: "function"; inner: Function }
    | { kind: "trait"; inner: Trait }
    | { kind: "trait_alias"; inner: TraitAlias }
    | { kind: "method"; inner: Method }
    | { kind: "impl"; inner: Impl }
    | { kind: "typedef"; inner: Typedef }
    | { kind: "opaque_ty"; inner: OpaqueTy }
    | { kind: "constant"; inner: Constant }
    | { kind: "static"; inner: Static }
    | { kind: "foreign_type"; inner: null }
    | { kind: "macro"; inner: string }
    | { kind: "proc_macro"; inner: ProcMacro }
    | {
          kind: "assoc_const";
          inner: {
              type: Type;
              default: string | null;
          };
      }
    | {
          kind: "assoc_type";
          inner: {
              bounds: GenericBound[];
              default: Type | null;
          };
      };

export type Variant =
    | { variant_kind: "plain"; variant_inner: null }
    | {
          variant_kind: "tuple";
          variant_inner: Type[];
      }
    | { variant_kind: "struct"; variant_inner: Id[] };

export type StructType = "plain" | "tuple" | "unit";

export type Qualifiers = "const" | "unsafe" | "async";

export type GenericParamDefKind =
    | "lifetime"
    | {
          bounds: GenericBound[];
          default: Type | null;
      }
    | {
          ty: Type;
          default: string | null;
      };

export type WherePredicate =
    | {
          ty: Type;
          bounds: GenericBound[];
      }
    | {
          lifetime: string;
          bounds: GenericBound[];
      }
    | {
          lhs: Type;
          rhs: Type;
      };

export type GenericBound =
    | {
          trait: Type;
          generic_params: GenericParamDef[];
          modifier: TraitBoundModifier;
      }
    | string;

export type TraitBoundModifier = "none" | "maybe" | "maybe_const";

export type Type =
    | {
          kind: "resolved_path";
          inner: {
              name: string;
              id: Id;
              args: GenericArgs | null;
              param_names: GenericBound[];
          };
      }
    | { kind: "generic"; inner: string }
    | { kind: "primitive"; inner: string }
    | { kind: "function_pointer"; inner: FunctionPointer }
    | { kind: "tuple"; inner: Type[] }
    | { kind: "slice"; inner: Type }
    | {
          kind: "array";
          inner: {
              type: Type;
              len: string;
          };
      }
    | { kind: "impl_trait"; inner: GenericBound[] }
    | { kind: "never"; inner: null }
    | { kind: "infer"; inner: null }
    | {
          kind: "raw_pointer";
          inner: {
              mutable: boolean;
              type: Type;
          };
      }
    | {
          kind: "borrowed_ref";
          inner: {
              lifetime: string | null;
              mutable: boolean;
              type: Type;
          };
      }
    | {
          kind: "qualified_path";
          inner: {
              name: string;
              self_type: Type;
              trait: Type;
          };
      };

export type MacroKind = "bang" | "attr" | "derive";

export interface Crate {
    root: Id;
    crate_version: string | null;
    includes_private: boolean;
    index: { [key: string]: Item };
    paths: { [key: string]: ItemSummary };
    external_crates: { [key: number]: ExternalCrate };
    format_version: number;
}

export const isExternCrate = (
    val: any,
): val is {
    kind: "extern_crate";
    inner: {
        name: string;
        rename: string | null;
    };
} & ItemBase => typeof val === "object" && val?.kind === "extern_crate";
export const isAssocConst = (
    val: any,
): val is {
    kind: "assoc_const";
    inner: {
        type: Type;
        default: string | null;
    };
} & ItemBase => typeof val === "object" && val?.kind === "assoc_const";
export const isAssocType = (
    val: any,
): val is {
    kind: "assoc_type";
    inner: {
        bounds: GenericBound[];
        default: Type | null;
    };
} & ItemBase => typeof val === "object" && val?.kind === "assoc_type";

export const isModule = (
    val: any,
): val is { kind: "module"; inner: Module } & ItemBase =>
    typeof val === "object" && val?.kind === "module";
export const isImport = (
    val: any,
): val is { kind: "import"; inner: Import } & ItemBase =>
    typeof val === "object" && val?.kind === "import";
export const isUnion = (
    val: any,
): val is { kind: "union"; inner: Union } & ItemBase =>
    typeof val === "object" && val?.kind === "union";
export const isStruct = (
    val: any,
): val is { kind: "struct"; inner: Struct } & ItemBase =>
    typeof val === "object" && val?.kind === "struct";
export const isStructField = (
    val: any,
): val is { kind: "struct_field"; inner: Type } & ItemBase =>
    typeof val === "object" && val?.kind === "struct_field";
export const isEnum = (
    val: any,
): val is { kind: "enum"; inner: Enum } & ItemBase =>
    typeof val === "object" && val?.kind === "enum";
export const isVariant = (
    val: any,
): val is { kind: "variant"; inner: Variant } & ItemBase =>
    typeof val === "object" && val?.kind === "variant";
export const isFunction = (
    val: any,
): val is { kind: "function"; inner: Function } & ItemBase =>
    typeof val === "object" && val?.kind === "function";
export const isTrait = (
    val: any,
): val is { kind: "trait"; inner: Trait } & ItemBase =>
    typeof val === "object" && val?.kind === "trait";
export const isTraitAlias = (
    val: any,
): val is { kind: "trait_alias"; inner: TraitAlias } & ItemBase =>
    typeof val === "object" && val?.kind === "trait_alias";
export const isMethod = (
    val: any,
): val is { kind: "method"; inner: Method } & ItemBase =>
    typeof val === "object" && val?.kind === "method";
export const isImpl = (
    val: any,
): val is { kind: "impl"; inner: Impl } & ItemBase =>
    typeof val === "object" && val?.kind === "impl";
export const isTypedef = (
    val: any,
): val is { kind: "typedef"; inner: Typedef } & ItemBase =>
    typeof val === "object" && val?.kind === "typedef";
export const isOpaqueTy = (
    val: any,
): val is { kind: "opaque_ty"; inner: OpaqueTy } & ItemBase =>
    typeof val === "object" && val?.kind === "opaque_ty";
export const isConstant = (
    val: any,
): val is { kind: "constant"; inner: Constant } & ItemBase =>
    typeof val === "object" && val?.kind === "constant";
export const isStatic = (
    val: any,
): val is { kind: "static"; inner: Static } & ItemBase =>
    typeof val === "object" && val?.kind === "static";
export const isForeignType = (
    val: any,
): val is { kind: "foreign_type"; inner: null } & ItemBase =>
    typeof val === "object" && val?.kind === "foreign_type";
export const isMacro = (
    val: any,
): val is { kind: "macro"; inner: string } & ItemBase =>
    typeof val === "object" && val?.kind === "macro";
export const isProcMacro = (
    val: any,
): val is { kind: "proc_macro"; inner: ProcMacro } & ItemBase =>
    typeof val === "object" && val?.kind === "proc_macro";
