export interface ExternalCrate {
    name: string;
    html_root_url: string | null;
}

export interface ItemSummary {
    crate_id: number;
    path: string[];
    kind: ItemKind;
}

export interface Item {
    id: Id;
    crate_id: number;
    name: string | null;
    span: Span | null;
    visibility: Visibility;
    docs: string | null;
    links: { [key: string]: Id };
    attrs: string[];
    deprecation: Deprecation | null;
    inner: ItemEnum;
}

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
