//! Scope tests over hand-built ASTs (the parsers are being ported in parallel). Tests that need
//! the real parser are `#[ignore = "needs parser"]` and carry their `.svelte` source.

use super::analyze::{ComponentScopeAnalysis, analyze_component_scopes};
use super::*;
use crate::svelte::ast::*;
use std::cell::Cell;

// ---------------------------------------------------------------------------------------------
// builders (every node gets a distinct, increasing span)

thread_local!(static POS: Cell<u32> = const { Cell::new(0) });

fn sp() -> (u32, u32) {
    POS.with(|p| {
        let s = p.get();
        p.set(s + 2);
        (s, s + 1)
    })
}

fn id(name: &str) -> Identifier {
    let (start, end) = sp();
    Identifier {
        start,
        end,
        name: name.into(),
    }
}
fn e_id(name: &str) -> Expression {
    Expression::Identifier(id(name))
}
fn p_id(name: &str) -> Pattern {
    Pattern::Identifier(id(name))
}
fn lit_num(n: f64) -> Expression {
    let (start, end) = sp();
    Expression::Literal(Literal {
        start,
        end,
        value: LiteralValue::Number(n),
        raw: n.to_string(),
        regex: None,
        bigint: None,
    })
}
fn lit_str(s: &str) -> Expression {
    let (start, end) = sp();
    Expression::Literal(Literal {
        start,
        end,
        value: LiteralValue::String(s.into()),
        raw: format!("'{s}'"),
        regex: None,
        bigint: None,
    })
}
fn str_node(s: &str) -> LiteralNode {
    let Expression::Literal(l) = lit_str(s) else {
        unreachable!()
    };
    LiteralNode::Literal(l)
}
fn call(callee: Expression, args: Vec<Expression>) -> Expression {
    let (start, end) = sp();
    Expression::CallExpression(CallExpression {
        start,
        end,
        callee: Box::new(callee),
        arguments: args.into_iter().map(ArrayElement::Expression).collect(),
        optional: false,
    })
}
fn member_expr(object: Expression, prop: &str) -> MemberExpression {
    let (start, end) = sp();
    MemberExpression {
        start,
        end,
        object: Box::new(object),
        property: PropertyKey::Expression(Box::new(e_id(prop))),
        computed: false,
        optional: false,
    }
}
fn member(object: Expression, prop: &str) -> Expression {
    Expression::MemberExpression(member_expr(object, prop))
}
fn bin(left: Expression, op: BinaryOperator, right: Expression) -> Expression {
    let (start, end) = sp();
    Expression::BinaryExpression(BinaryExpression {
        start,
        end,
        left: BinaryLeft::Expression(Box::new(left)),
        operator: op,
        right: Box::new(right),
    })
}
fn assign(left: Pattern, right: Expression) -> Expression {
    let (start, end) = sp();
    Expression::AssignmentExpression(AssignmentExpression {
        start,
        end,
        operator: AssignmentOperator::Assign,
        left,
        right: Box::new(right),
    })
}
fn update(argument: Expression) -> Expression {
    let (start, end) = sp();
    Expression::UpdateExpression(UpdateExpression {
        start,
        end,
        operator: UpdateOperator::Increment,
        prefix: false,
        argument: Box::new(argument),
    })
}
fn template(quasis: &[&str], expressions: Vec<Expression>) -> Expression {
    let (start, end) = sp();
    let n = quasis.len();
    Expression::TemplateLiteral(TemplateLiteral {
        start,
        end,
        expressions,
        quasis: quasis
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let (start, end) = sp();
                TemplateElement {
                    start,
                    end,
                    value: TemplateValue {
                        raw: q.to_string(),
                        cooked: Some(q.to_string()),
                    },
                    tail: i == n - 1,
                }
            })
            .collect(),
    })
}
fn array_pattern(names: &[&str]) -> Pattern {
    let (start, end) = sp();
    Pattern::ArrayPattern(ArrayPattern {
        start,
        end,
        elements: names.iter().map(|n| Some(p_id(n))).collect(),
    })
}
fn array(elements: Vec<Expression>) -> Expression {
    let (start, end) = sp();
    Expression::ArrayExpression(ArrayExpression {
        start,
        end,
        elements: elements
            .into_iter()
            .map(|e| Some(ArrayElement::Expression(e)))
            .collect(),
    })
}
/// `{ key: value }` pattern members; `None` value = shorthand.
fn obj_pattern(props: Vec<(&str, Pattern)>, rest: Option<&str>) -> Pattern {
    let (start, end) = sp();
    let mut properties: Vec<ObjectPatternMember> = props
        .into_iter()
        .map(|(k, v)| {
            let (start, end) = sp();
            ObjectPatternMember::Property(PatternProperty {
                start,
                end,
                method: false,
                shorthand: true,
                computed: false,
                key: PropertyKey::Expression(Box::new(e_id(k))),
                value: v,
                kind: PropertyKind::Init,
            })
        })
        .collect();
    if let Some(r) = rest {
        let (start, end) = sp();
        properties.push(ObjectPatternMember::RestElement(RestElement {
            start,
            end,
            argument: Box::new(p_id(r)),
        }));
    }
    Pattern::ObjectPattern(ObjectPattern { start, end, properties })
}
fn default_pattern(name: &str, value: Expression) -> Pattern {
    let (start, end) = sp();
    Pattern::AssignmentPattern(AssignmentPattern {
        start,
        end,
        left: Box::new(p_id(name)),
        right: Box::new(value),
    })
}
fn var_decl(kind: VariableKind, target: Pattern, init: Option<Expression>) -> VariableDeclaration {
    let (start, end) = sp();
    let (ds, de) = sp();
    VariableDeclaration {
        start,
        end,
        kind,
        declarations: vec![VariableDeclarator {
            start: ds,
            end: de,
            id: target,
            init,
        }],
    }
}
fn var(kind: VariableKind, target: Pattern, init: Option<Expression>) -> Statement {
    Statement::VariableDeclaration(var_decl(kind, target, init))
}
fn let_(name: &str, init: Expression) -> Statement {
    var(VariableKind::Let, p_id(name), Some(init))
}
fn const_(name: &str, init: Expression) -> Statement {
    var(VariableKind::Const, p_id(name), Some(init))
}
fn stmt(e: Expression) -> Statement {
    let (start, end) = sp();
    Statement::ExpressionStatement(ExpressionStatement {
        start,
        end,
        expression: e,
        directive: None,
    })
}
fn block_stmt(body: Vec<Statement>) -> BlockStatement {
    let (start, end) = sp();
    BlockStatement { start, end, body }
}
fn block(body: Vec<Statement>) -> Statement {
    Statement::BlockStatement(block_stmt(body))
}
fn ret(e: Expression) -> Statement {
    let (start, end) = sp();
    Statement::ReturnStatement(ReturnStatement {
        start,
        end,
        argument: Some(e),
    })
}
fn function(name: Option<&str>, params: Vec<Pattern>, body: Vec<Statement>) -> Function {
    let (start, end) = sp();
    Function {
        start,
        end,
        id: name.map(id),
        expression: false,
        generator: false,
        is_async: false,
        params,
        body: BlockStatementNode::BlockStatement(block_stmt(body)),
    }
}
fn func_decl(name: &str, params: Vec<Pattern>, body: Vec<Statement>) -> Statement {
    Statement::FunctionDeclaration(function(Some(name), params, body))
}
fn arrow(params: Vec<Pattern>, body: Expression) -> Expression {
    let (start, end) = sp();
    Expression::ArrowFunctionExpression(ArrowFunctionExpression {
        start,
        end,
        id: None,
        expression: true,
        generator: false,
        is_async: false,
        params,
        body: ArrowBody::Expression(Box::new(body)),
    })
}
fn try_catch(body: Vec<Statement>, param: &str, handler: Vec<Statement>) -> Statement {
    let (start, end) = sp();
    let (cs, ce) = sp();
    Statement::TryStatement(TryStatement {
        start,
        end,
        block: block_stmt(body),
        handler: Some(CatchClause {
            start: cs,
            end: ce,
            param: Some(p_id(param)),
            body: block_stmt(handler),
        }),
        finalizer: None,
    })
}
fn labeled(label: &str, body: Statement) -> Statement {
    let (start, end) = sp();
    Statement::LabeledStatement(LabeledStatement {
        start,
        end,
        label: id(label),
        body: Box::new(body),
    })
}
fn import(specs: &[(&str, &str)], source: &str) -> Statement {
    let (start, end) = sp();
    Statement::ImportDeclaration(ImportDeclaration {
        start,
        end,
        specifiers: specs
            .iter()
            .map(|(imported, local)| {
                let (start, end) = sp();
                if *imported == "default" {
                    ImportSpecifierKind::ImportDefaultSpecifier(ImportDefaultSpecifier {
                        start,
                        end,
                        local: id(local),
                    })
                } else {
                    ImportSpecifierKind::ImportSpecifier(ImportSpecifier {
                        start,
                        end,
                        imported: ModuleExportName::Identifier(id(imported)),
                        local: id(local),
                    })
                }
            })
            .collect(),
        source: str_node(source),
        attributes: Vec::new(),
    })
}
fn program(body: Vec<Statement>) -> Program {
    let (start, end) = sp();
    Program {
        start,
        end,
        body,
        source_type: SourceType::Module,
    }
}
fn script(context: ScriptContext, body: Vec<Statement>) -> Script {
    let (start, end) = sp();
    Script {
        start,
        end,
        context,
        content: program(body),
        attributes: Vec::new(),
    }
}

// template

fn frag(nodes: Vec<FragmentNode>) -> Fragment {
    Fragment { nodes }
}
fn tag(e: Expression) -> FragmentNode {
    let (start, end) = sp();
    FragmentNode::ExpressionTag(ExpressionTag {
        start,
        end,
        expression: e,
    })
}
fn const_tag(name: &str, init: Expression) -> FragmentNode {
    let (start, end) = sp();
    FragmentNode::ConstTag(ConstTag {
        start,
        end,
        declaration: var_decl(VariableKind::Const, p_id(name), Some(init)),
    })
}
fn render(e: Expression) -> FragmentNode {
    let (start, end) = sp();
    FragmentNode::RenderTag(RenderTag {
        start,
        end,
        expression: e,
    })
}
fn each(
    expression: Expression,
    context: Option<Pattern>,
    index: Option<&str>,
    key: Option<Expression>,
    body: Vec<FragmentNode>,
) -> FragmentNode {
    let (start, end) = sp();
    FragmentNode::EachBlock(EachBlock {
        start,
        end,
        expression,
        context,
        body: frag(body),
        fallback: None,
        index: index.map(String::from),
        key,
    })
}
fn if_block(test: Expression, consequent: Vec<FragmentNode>) -> FragmentNode {
    let (start, end) = sp();
    FragmentNode::IfBlock(IfBlock {
        start,
        end,
        elseif: false,
        test,
        consequent: frag(consequent),
        alternate: None,
    })
}
fn snippet(name: &str, parameters: Vec<Pattern>, body: Vec<FragmentNode>) -> FragmentNode {
    let (start, end) = sp();
    FragmentNode::SnippetBlock(SnippetBlock {
        start,
        end,
        expression: id(name),
        parameters,
        type_params: None,
        body: frag(body),
    })
}
fn element(name: &str, attributes: Vec<AttributeLike>, children: Vec<FragmentNode>) -> Element {
    let (start, end) = sp();
    Element {
        start,
        end,
        name: name.into(),
        name_span: Span::default(),
        attributes,
        fragment: frag(children),
    }
}
fn regular(name: &str, attributes: Vec<AttributeLike>, children: Vec<FragmentNode>) -> FragmentNode {
    FragmentNode::RegularElement(element(name, attributes, children))
}
fn component(name: &str, attributes: Vec<AttributeLike>, children: Vec<FragmentNode>) -> FragmentNode {
    FragmentNode::Component(element(name, attributes, children))
}
fn directive(name: &str, expression: Option<Expression>) -> Directive {
    let (start, end) = sp();
    Directive {
        start,
        end,
        name: name.into(),
        name_span: None,
        expression,
        modifiers: Vec::new(),
    }
}
fn bind(name: &str, e: Expression) -> AttributeLike {
    AttributeLike::BindDirective(directive(name, Some(e)))
}
fn on(name: &str, e: Expression) -> AttributeLike {
    AttributeLike::OnDirective(directive(name, Some(e)))
}
fn let_dir(name: &str, e: Option<Expression>) -> AttributeLike {
    AttributeLike::LetDirective(directive(name, e))
}
fn root(module: Option<Vec<Statement>>, instance: Option<Vec<Statement>>, nodes: Vec<FragmentNode>) -> Root {
    let (start, end) = sp();
    Root {
        start,
        end,
        options: None,
        fragment: frag(nodes),
        css: None,
        instance: instance.map(|b| script(ScriptContext::Default, b)),
        module: module.map(|b| script(ScriptContext::Module, b)),
        comments: Vec::new(),
        ts: false,
    }
}

// queries

/// All Identifier nodes named `name`, in walk order (instance script, module script, template —
/// see `walk::NodeRef::Root` children order: fragment first, then instance, then module).
fn idents<'a>(r: &'a Root, name: &str) -> Vec<&'a Identifier> {
    struct Collect<'a, 'n> {
        name: &'n str,
        out: Vec<&'a Identifier>,
    }
    impl<'a> Visitor<'a> for Collect<'a, '_> {
        type State = ();
        fn visit(&mut self, node: NodeRef<'a>, state: &(), w: &mut Walker<'a>) {
            if let NodeRef::Identifier(i) = node
                && i.name == self.name
            {
                self.out.push(i);
            }
            w.next(self, node, state);
        }
    }
    let mut c = Collect { name, out: Vec::new() };
    Walker::walk(NodeRef::Root(r), &(), &mut c);
    c.out
}

fn analyze(r: &Root) -> ComponentScopeAnalysis<'_> {
    analyze_component_scopes(r, None).expect("scope analysis")
}

fn binding_named<'a, 'b>(a: &'b ComponentScopeAnalysis<'a>, scope: ScopeId, name: &str) -> &'b Binding<'a> {
    let b = a.arena.get(scope, name).unwrap_or_else(|| panic!("no binding {name}"));
    a.arena.binding(b)
}

fn kind_of(a: &ComponentScopeAnalysis<'_>, i: &Identifier) -> Option<BindingKind> {
    a.arena.resolve(i).map(|b| a.arena.binding(b).kind)
}

// ---------------------------------------------------------------------------------------------
// tests

/// ```svelte
/// <script>
///   let count = $state(0);
///   let list = $state.raw([]);
///   const double = $derived(count * 2);
///   const triple = $derived.by(() => count * 3);
///   let { a, b = 1, c = $bindable(), ...rest } = $props();
///   function inc() { count++; }
/// </script>
/// <button onclick={inc}>{count} {double} {a} {rest}</button>
/// ```
fn runes_root() -> Root {
    root(
        None,
        Some(vec![
            let_("count", call(e_id("$state"), vec![lit_num(0.0)])),
            let_("list", call(member(e_id("$state"), "raw"), vec![array(vec![])])),
            const_(
                "double",
                call(
                    e_id("$derived"),
                    vec![bin(e_id("count"), BinaryOperator::Mul, lit_num(2.0))],
                ),
            ),
            const_(
                "triple",
                call(
                    member(e_id("$derived"), "by"),
                    vec![arrow(vec![], bin(e_id("count"), BinaryOperator::Mul, lit_num(3.0)))],
                ),
            ),
            var(
                VariableKind::Let,
                obj_pattern(
                    vec![
                        ("a", p_id("a")),
                        ("b", default_pattern("b", lit_num(1.0))),
                        ("c", default_pattern("c", call(e_id("$bindable"), vec![]))),
                    ],
                    Some("rest"),
                ),
                Some(call(e_id("$props"), vec![])),
            ),
            func_decl("inc", vec![], vec![stmt(update(e_id("count")))]),
        ]),
        vec![regular(
            "button",
            vec![on("click", e_id("inc"))],
            vec![
                tag(e_id("count")),
                tag(e_id("double")),
                tag(e_id("a")),
                tag(e_id("rest")),
            ],
        )],
    )
}

#[test]
fn rune_binding_kinds() {
    let r = runes_root();
    let a = analyze(&r);
    assert!(a.runes, "rune references opt into runes mode");
    let s = a.instance.scope;

    assert_eq!(binding_named(&a, s, "count").kind, BindingKind::State);
    assert!(binding_named(&a, s, "count").reassigned, "count++ inside inc()");
    assert_eq!(binding_named(&a, s, "list").kind, BindingKind::RawState);
    assert_eq!(binding_named(&a, s, "double").kind, BindingKind::Derived);
    assert_eq!(binding_named(&a, s, "triple").kind, BindingKind::Derived);
    assert_eq!(binding_named(&a, s, "double").declaration_kind, DeclarationKind::Const);

    let pa = binding_named(&a, s, "a");
    assert_eq!(pa.kind, BindingKind::Prop);
    assert_eq!(pa.prop_alias.as_deref(), Some("a"));
    assert!(pa.initial.is_none());

    let pb = binding_named(&a, s, "b");
    assert_eq!(pb.kind, BindingKind::Prop);
    assert!(
        matches!(pb.initial, Some(Initial::Expression(Expression::Literal(_)))),
        "initial rewired to the fallback"
    );

    let pc = binding_named(&a, s, "c");
    assert_eq!(pc.kind, BindingKind::BindableProp);
    assert!(pc.initial.is_none(), "$bindable() without argument");

    let rest = binding_named(&a, s, "rest");
    assert_eq!(rest.kind, BindingKind::RestProp);
    assert_eq!(
        rest.metadata.as_ref().and_then(|m| m.exclude_props.clone()),
        Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
    );

    assert_eq!(binding_named(&a, s, "inc").declaration_kind, DeclarationKind::Function);
    assert!(binding_named(&a, s, "inc").is_function());

    // template references resolve into the instance scope
    for (name, kind) in [
        ("count", BindingKind::State),
        ("double", BindingKind::Derived),
        ("a", BindingKind::Prop),
        ("rest", BindingKind::RestProp),
    ] {
        let ids = idents(&r, name);
        // walk order is template first
        assert_eq!(kind_of(&a, ids[0]), Some(kind), "{name} in template");
    }
    let inc = idents(&r, "inc");
    assert_eq!(kind_of(&a, inc[0]), Some(BindingKind::Normal));

    // runes are globals: unresolved
    for i in idents(&r, "$state") {
        assert_eq!(a.arena.resolve(i), None);
    }
}

#[test]
fn props_identifier_is_rest_prop() {
    let r = root(None, Some(vec![let_("props", call(e_id("$props"), vec![]))]), vec![]);
    let a = analyze(&r);
    let p = binding_named(&a, a.instance.scope, "props");
    assert_eq!(p.kind, BindingKind::RestProp);
    assert!(p.initial.is_none());
}

#[test]
fn props_invalid_identifier() {
    let r = root(
        None,
        Some(vec![var(
            VariableKind::Let,
            array_pattern(&["x"]),
            Some(call(e_id("$props"), vec![])),
        )]),
        vec![],
    );
    let err = analyze_component_scopes(&r, None).unwrap_err();
    assert_eq!(err.code, "props_invalid_identifier");
    assert_eq!(
        err.message,
        "`$props()` can only be used with an object destructuring pattern"
    );
}

#[test]
fn runes_false_treats_rune_names_as_stores() {
    // with `runes: false`, `$state` is a subscription to a store `state` (upstream behaviour), so
    // the VariableDeclarator `rune_invalid_usage` check does not fire
    let r = root(
        None,
        Some(vec![let_("x", call(e_id("$state"), vec![lit_num(0.0)]))]),
        vec![],
    );
    let a = analyze_component_scopes(&r, Some(false)).unwrap();
    assert!(!a.runes);
    assert_eq!(kind_of(&a, idents(&r, "$state")[0]), Some(BindingKind::StoreSub));
    assert_eq!(binding_named(&a, a.instance.scope, "x").kind, BindingKind::Normal);
}

/// ```js
/// let x = 1;
/// function f(x) { return x; }
/// { let x = 2; x; }
/// try {} catch (x) { x; }
/// const g = (x) => x;
/// x;
/// ```
#[test]
fn shadowing_in_functions_blocks_and_catch() {
    let r = root(
        None,
        Some(vec![
            let_("x", lit_num(1.0)),
            func_decl("f", vec![p_id("x")], vec![ret(e_id("x"))]),
            block(vec![let_("x", lit_num(2.0)), stmt(e_id("x"))]),
            try_catch(vec![], "x", vec![stmt(e_id("x"))]),
            const_("g", arrow(vec![p_id("x")], e_id("x"))),
            stmt(e_id("x")),
        ]),
        vec![tag(e_id("x"))],
    );
    let a = analyze(&r);
    let ids = idents(&r, "x");
    // template {x}, then: decl, param, use, block decl, block use, catch param, catch use, arrow param, arrow use, top use
    assert_eq!(ids.len(), 11);
    let res: Vec<_> = ids.iter().map(|i| a.arena.resolve(i).expect("resolved")).collect();
    let outer = res[1];
    assert_eq!(res[0], outer, "template {{x}}");
    assert_eq!(res[10], outer, "top-level use");
    assert_eq!(res[2], res[3], "param and its use");
    assert_ne!(res[2], outer);
    assert_eq!(a.arena.binding(res[2]).declaration_kind, DeclarationKind::Param);
    assert_eq!(res[4], res[5], "block-scoped let");
    assert_ne!(res[4], outer);
    assert_eq!(res[6], res[7], "catch param");
    assert_ne!(res[6], outer);
    assert_eq!(res[8], res[9], "arrow param");
    assert_ne!(res[8], res[2]);

    // function scopes: the params scope is porous, the body block is the function scope
    let param_scope = a.arena.binding(res[2]).scope;
    assert!(a.arena.scope(param_scope).is_porous());
    assert_eq!(
        a.arena.scope(param_scope).function_depth,
        a.arena.scope(a.instance.scope).function_depth
    );
}

#[test]
fn var_hoists_out_of_blocks() {
    let r = root(
        None,
        Some(vec![
            block(vec![var(VariableKind::Var, p_id("v"), Some(lit_num(1.0)))]),
            stmt(e_id("v")),
        ]),
        vec![],
    );
    let a = analyze(&r);
    let v = a.arena.get(a.instance.scope, "v").expect("hoisted");
    assert_eq!(a.arena.binding(v).scope, a.instance.scope);
    assert_eq!(a.arena.binding(v).declaration_kind, DeclarationKind::Var);
}

/// ```svelte
/// <script> let items = $state([]); </script>
/// {#each items as item, i (item.id)}{item}{i}{/each}
/// {#each items as row, j}{j}{/each}
/// {item}
/// ```
#[test]
fn each_block_context_and_index() {
    let r = root(
        None,
        Some(vec![let_("items", call(e_id("$state"), vec![array(vec![])]))]),
        vec![
            each(
                e_id("items"),
                Some(p_id("item")),
                Some("i"),
                Some(member(e_id("item"), "id")),
                vec![tag(e_id("item")), tag(e_id("i"))],
            ),
            each(e_id("items"), Some(p_id("row")), Some("j"), None, vec![tag(e_id("j"))]),
            tag(e_id("item")),
        ],
    );
    let a = analyze(&r);
    let items = idents(&r, "items");
    assert_eq!(kind_of(&a, items[0]), Some(BindingKind::State));

    let item = idents(&r, "item");
    // context decl, key object, body use, outside use
    assert_eq!(item.len(), 4);
    assert_eq!(kind_of(&a, item[0]), Some(BindingKind::Each));
    assert_eq!(
        a.arena.resolve(item[1]),
        a.arena.resolve(item[0]),
        "key expression sees the context"
    );
    assert_eq!(a.arena.resolve(item[2]), a.arena.resolve(item[0]));
    assert_eq!(a.arena.resolve(item[3]), None, "context is not visible after the block");

    let i = idents(&r, "i");
    let ib = a.arena.binding(a.arena.resolve(i[0]).unwrap());
    assert_eq!(
        ib.kind,
        BindingKind::Template,
        "keyed by something other than the index"
    );
    assert!(matches!(ib.initial, Some(Initial::EachBlock(_))));
    let each_scope = a.arena.binding(a.arena.resolve(item[0]).unwrap()).scope;
    let i_expr = Expression::Identifier(i[0].clone());
    assert!(
        a.arena.evaluate(each_scope, &i_expr).is_number,
        "each index evaluates to a number"
    );

    let j = idents(&r, "j");
    assert_eq!(kind_of(&a, j[0]), Some(BindingKind::Static));

    assert_eq!(a.template.each_block_index.len(), 2);
    let mut names: Vec<_> = a.template.each_block_index.values().cloned().collect();
    names.sort();
    assert_eq!(names, ["$$index", "$$index_1"]);
}

#[test]
fn each_rest_metadata() {
    let r = root(
        None,
        Some(vec![]),
        vec![each(
            e_id("xs"),
            Some(obj_pattern(vec![("a", p_id("a"))], Some("others"))),
            None,
            None,
            vec![],
        )],
    );
    let a = analyze(&r);
    let a_id = idents(&r, "a");
    let others = idents(&r, "others");
    let meta = |i: &Identifier| {
        a.arena
            .binding(a.arena.resolve(i).unwrap())
            .metadata
            .clone()
            .unwrap()
            .inside_rest
    };
    assert_eq!(meta(a_id.last().unwrap()), Some(false));
    assert_eq!(meta(others[0]), Some(true));
}

/// ```svelte
/// {#snippet row(item, extra)}{item}{@const doubled = item * 2}{doubled}{/snippet}
/// {@render row(1)}
/// {item}
/// ```
#[test]
fn snippet_params_and_const_tags() {
    let r = root(
        None,
        Some(vec![]),
        vec![
            snippet(
                "row",
                vec![p_id("item"), p_id("extra")],
                vec![
                    tag(e_id("item")),
                    const_tag("doubled", bin(e_id("item"), BinaryOperator::Mul, lit_num(2.0))),
                    tag(e_id("doubled")),
                ],
            ),
            render(call(e_id("row"), vec![lit_num(1.0)])),
            tag(e_id("item")),
        ],
    );
    let a = analyze(&r);
    let row = idents(&r, "row");
    let rb = a.arena.binding(a.arena.resolve(row[0]).unwrap());
    assert_eq!(rb.declaration_kind, DeclarationKind::Function);
    assert!(matches!(rb.initial, Some(Initial::SnippetBlock(_))));
    assert_eq!(a.arena.resolve(row[1]), a.arena.resolve(row[0]), "{{@render row()}}");

    let item = idents(&r, "item");
    assert_eq!(kind_of(&a, item[0]), Some(BindingKind::Snippet));
    assert_eq!(a.arena.resolve(item[1]), a.arena.resolve(item[0]));
    assert_eq!(
        a.arena.resolve(item[2]),
        a.arena.resolve(item[0]),
        "inside the const tag"
    );
    assert_eq!(a.arena.resolve(item[3]), None, "outside the snippet");

    let doubled = idents(&r, "doubled");
    let db = a.arena.binding(a.arena.resolve(doubled[0]).unwrap());
    assert_eq!(db.kind, BindingKind::Template);
    assert_eq!(db.declaration_kind, DeclarationKind::Const);
    assert_eq!(a.arena.resolve(doubled[1]), a.arena.resolve(doubled[0]));
}

#[test]
fn snippet_param_body_duplicate() {
    let r = root(
        None,
        None,
        vec![snippet("s", vec![p_id("x")], vec![const_tag("x", lit_num(1.0))])],
    );
    let err = analyze_component_scopes(&r, None).unwrap_err();
    assert_eq!(err.code, "declaration_duplicate");
    assert_eq!(err.message, "`x` has already been declared");
}

#[test]
fn const_tag_in_if_block_and_element() {
    let r = root(
        None,
        Some(vec![let_("x", lit_num(1.0))]),
        vec![
            if_block(
                e_id("x"),
                vec![
                    const_tag("y", bin(e_id("x"), BinaryOperator::Mul, lit_num(2.0))),
                    tag(e_id("y")),
                ],
            ),
            regular("div", vec![], vec![tag(e_id("y"))]),
        ],
    );
    let a = analyze(&r);
    let y = idents(&r, "y");
    assert_eq!(kind_of(&a, y[0]), Some(BindingKind::Template));
    assert_eq!(a.arena.resolve(y[1]), a.arena.resolve(y[0]));
    assert_eq!(a.arena.resolve(y[2]), None);
}

#[test]
fn imports_are_declared_in_the_module_scope() {
    let r = root(
        Some(vec![import(&[("default", "Thing")], "./Thing.svelte")]),
        Some(vec![
            import(&[("format", "fmt")], "./util.js"),
            stmt(call(e_id("fmt"), vec![])),
        ]),
        vec![component("Thing", vec![], vec![]), tag(call(e_id("fmt"), vec![]))],
    );
    let a = analyze(&r);
    let fmt = a.arena.get(a.instance.scope, "fmt").unwrap();
    let fb = a.arena.binding(fmt);
    assert_eq!(fb.declaration_kind, DeclarationKind::Import);
    // upstream: `declare` forwards imports to the parent scope (instance -> module)
    assert_eq!(fb.scope, a.module.scope);
    assert!(matches!(fb.initial, Some(Initial::ImportDeclaration(_))));
    // `format` (the imported name) is not a reference
    assert!(idents(&r, "format").iter().all(|i| a.arena.resolve(i).is_none()));
    // walk order: template call, import specifier local (not visited: no reference), script call
    let ids = idents(&r, "fmt");
    assert_eq!(a.arena.resolve(ids[0]), Some(fmt));
    assert_eq!(a.arena.resolve(ids[1]), None);
    assert_eq!(a.arena.resolve(ids[2]), Some(fmt));
    // the component tag references the import (synthetic identifier)
    let thing = a.arena.get(a.module.scope, "Thing").unwrap();
    assert_eq!(a.arena.binding(thing).references.len(), 1);
}

#[test]
fn reassigned_and_mutated() {
    let r = root(
        None,
        Some(vec![
            let_("a", lit_num(1.0)),
            let_("b", lit_num(2.0)),
            let_("obj", lit_num(0.0)),
            let_("name", lit_str("")),
            let_("untouched", lit_num(0.0)),
            stmt(assign(array_pattern(&["a", "b"]), array(vec![e_id("b"), e_id("a")]))),
            stmt(assign(
                Pattern::MemberExpression(Box::new(member_expr(member(e_id("obj"), "x"), "y"))),
                lit_num(1.0),
            )),
        ]),
        vec![regular("input", vec![bind("value", e_id("name"))], vec![])],
    );
    let a = analyze(&r);
    let s = a.instance.scope;
    assert!(binding_named(&a, s, "a").reassigned);
    assert!(binding_named(&a, s, "b").reassigned);
    assert_eq!(
        binding_named(&a, s, "a").assignments.len(),
        2,
        "initial + destructuring"
    );
    assert!(binding_named(&a, s, "obj").mutated);
    assert!(!binding_named(&a, s, "obj").reassigned);
    assert!(binding_named(&a, s, "name").reassigned, "bind:value");
    assert!(!binding_named(&a, s, "untouched").updated());
    // legacy mode (no runes): $$props and $$restProps exist
    assert!(!a.runes);
    assert_eq!(binding_named(&a, s, "$$props").kind, BindingKind::RestProp);
}

#[test]
fn let_directive_on_component() {
    let r = root(
        None,
        Some(vec![]),
        vec![component(
            "List",
            vec![let_dir("item", None), let_dir("x", Some(e_id("y")))],
            vec![tag(e_id("item")), tag(e_id("y"))],
        )],
    );
    let a = analyze(&r);
    let item = idents(&r, "item");
    assert_eq!(kind_of(&a, item[0]), Some(BindingKind::Template));
    let y = idents(&r, "y");
    // y in the directive, then in the body
    assert_eq!(kind_of(&a, y[1]), Some(BindingKind::Template));
    let comp = NodeRef::Component(match &r.fragment.nodes[0] {
        FragmentNode::Component(c) => c,
        _ => unreachable!(),
    });
    let cs = &a.template.component_scopes[&comp.key()];
    assert!(a.arena.scope(cs.default).declarations.contains_key("item"));
}

#[test]
fn legacy_reactive_and_store_subscriptions() {
    // let count = 0; $: doubled = count * 2;  import { writable } from 'svelte/store'; const s = writable(0);
    let r = root(
        None,
        Some(vec![
            import(&[("writable", "writable")], "svelte/store"),
            let_("count", lit_num(0.0)),
            const_("s", call(e_id("writable"), vec![lit_num(0.0)])),
            labeled(
                "$",
                stmt(assign(
                    p_id("doubled"),
                    bin(e_id("count"), BinaryOperator::Mul, lit_num(2.0)),
                )),
            ),
        ]),
        vec![tag(e_id("$s")), tag(e_id("doubled"))],
    );
    let a = analyze(&r);
    assert!(!a.runes);
    let s = a.instance.scope;
    assert_eq!(binding_named(&a, s, "doubled").kind, BindingKind::LegacyReactive);
    let store = idents(&r, "$s");
    let sb = a.arena.binding(a.arena.resolve(store[0]).expect("store_sub"));
    assert_eq!(sb.kind, BindingKind::StoreSub);
    assert_eq!(sb.declaration_kind, DeclarationKind::Synthetic);
    assert!(!a.arena.scope(a.module.scope).references.contains_key("$s"));
}

#[test]
fn scoped_store_subscription_errors() {
    // {#each xs as s}{$s}{/each} with a top-level store `s`... nested owner -> error
    let r = root(
        None,
        Some(vec![const_("s", call(e_id("writable"), vec![]))]),
        vec![each(e_id("xs"), Some(p_id("s")), None, None, vec![tag(e_id("$s"))])],
    );
    let err = analyze_component_scopes(&r, None).unwrap_err();
    assert_eq!(err.code, "store_invalid_scoped_subscription");
}

#[test]
fn duplicate_and_dollar_errors() {
    let r = root(
        None,
        Some(vec![let_("a", lit_num(1.0)), let_("a", lit_num(2.0))]),
        vec![],
    );
    let err = analyze_component_scopes(&r, None).unwrap_err();
    assert_eq!(err.code, "declaration_duplicate");

    let r = root(None, Some(vec![let_("$foo", lit_num(1.0))]), vec![]);
    let err = analyze_component_scopes(&r, None).unwrap_err();
    assert_eq!(
        (err.code, err.message.as_str()),
        (
            "dollar_prefix_invalid",
            "The $ prefix is reserved, and cannot be used for variables and imports"
        )
    );

    // var redeclaration is fine
    let r = root(
        None,
        Some(vec![
            var(VariableKind::Var, p_id("v"), None),
            var(VariableKind::Var, p_id("v"), None),
        ]),
        vec![],
    );
    assert!(analyze_component_scopes(&r, None).is_ok());

    // `$` prefix allowed in nested functions in legacy mode (function_depth > 1)
    let r = root(
        None,
        Some(vec![func_decl("f", vec![], vec![let_("$x", lit_num(1.0))])]),
        vec![],
    );
    assert!(analyze_component_scopes(&r, Some(false)).is_ok());
}

#[test]
fn evaluation() {
    let r = root(
        None,
        Some(vec![
            const_("n", lit_num(2.0)),
            const_("s", template(&["a", ""], vec![e_id("n")])),
            const_("m", call(member(e_id("Math"), "max"), vec![lit_num(1.0), e_id("n")])),
            let_("st", call(e_id("$state"), vec![lit_num(5.0)])),
            let_("u", lit_num(0.0)),
            stmt(update(e_id("u"))),
        ]),
        vec![],
    );
    let a = analyze(&r);
    let s = a.instance.scope;
    let init = |name: &str| {
        a.arena
            .binding(a.arena.get(s, name).unwrap())
            .initial
            .unwrap()
            .as_expression()
            .unwrap()
    };
    let ev = a.arena.evaluate(s, init("s"));
    assert!(ev.is_known && ev.is_string);
    assert!(matches!(&ev.value, js::JsValue::String(v) if v == "a2"));
    let ev = a.arena.evaluate(s, init("m"));
    assert!(matches!(ev.value, js::JsValue::Number(v) if v == 2.0) && ev.is_known);
    let ev = a.arena.evaluate(s, init("st"));
    assert!(matches!(ev.value, js::JsValue::Number(v) if v == 5.0), "$state(5)");
    let u_ref = idents(&r, "u");
    let u_expr = Expression::Identifier(u_ref[0].clone());
    let ev = a.arena.evaluate(s, &u_expr);
    assert!(!ev.is_known && ev.has_unknown, "reassigned binding");
    assert!(get_rune(&a.arena, Some(init("st")), s) == Some("$state"));
    assert!(!should_proxy(&a.arena, init("n"), Some(s)));
}

#[test]
fn generate_and_unique() {
    let mut arena = ScopeArena::new();
    let top = arena.new_scope(None, false);
    let child = arena.child(top, true);
    assert_eq!(arena.generate(child, "foo-bar"), "foo_bar");
    assert_eq!(arena.generate(top, "foo-bar"), "foo_bar_1");
    assert_eq!(arena.generate(top, "1x"), "_x");
    assert_eq!(arena.generate(top, "class"), "class_1");
    assert_eq!(arena.root.unique("$$index"), "$$index");
    assert_eq!(arena.root.unique("$$index"), "$$index_1");
}

// ---------------------------------------------------------------------------------------------
// with the real parser (enable once `svelte::parse` lands)

fn parse_and_analyze(src: &str, f: impl FnOnce(&Root, &ComponentScopeAnalysis<'_>)) {
    let r = crate::svelte::parse::parse(src, &Default::default()).expect("parse");
    let a = analyze_component_scopes(&r, None).expect("analyze");
    f(&r, &a);
}

#[test]
#[ignore = "needs parser"]
fn parsed_runes_component() {
    parse_and_analyze(
        r#"<script>
	let { label = 'x', value = $bindable(0), ...rest } = $props();
	let count = $state(0);
	const double = $derived(count * 2);
	function inc(count) { return count + 1; }
</script>

{#each [1, 2] as item, i (item)}
	{@const sum = item + i}
	<button onclick={() => count = inc(count)} {...rest}>{label} {value} {sum} {double}</button>
{/each}
{#snippet cell(v)}{v}{count}{/snippet}
{@render cell(count)}"#,
        |r, a| {
            assert!(a.runes);
            let s = a.instance.scope;
            assert_eq!(binding_named(a, s, "value").kind, BindingKind::BindableProp);
            assert_eq!(binding_named(a, s, "label").kind, BindingKind::Prop);
            assert_eq!(binding_named(a, s, "rest").kind, BindingKind::RestProp);
            assert_eq!(binding_named(a, s, "count").kind, BindingKind::State);
            assert!(binding_named(a, s, "count").reassigned);
            let count = idents(r, "count");
            let state = a.arena.get(s, "count");
            // decl, function param, param use, template uses
            assert_eq!(a.arena.resolve(count[0]), state);
            assert!(
                count.iter().filter(|i| a.arena.resolve(i) != state).count() == 2,
                "param + its use"
            );
            let sum = idents(r, "sum");
            assert_eq!(kind_of(a, sum[0]), Some(BindingKind::Template));
            assert_eq!(kind_of(a, idents(r, "v")[0]), Some(BindingKind::Snippet));
        },
    );
}

#[test]
#[ignore = "needs parser"]
fn parsed_await_block_scopes() {
    parse_and_analyze(
        r#"<script>let p = $state(Promise.resolve(1));</script>
{#await p then value}{value}{:catch error}{error}{/await}{value}"#,
        |r, a| {
            let value = idents(r, "value");
            assert_eq!(kind_of(a, value[1]), Some(BindingKind::Template));
            assert_eq!(a.arena.resolve(value.last().unwrap()), None);
            let error = idents(r, "error");
            assert_eq!(kind_of(a, error[1]), Some(BindingKind::Template));
        },
    );
}

#[test]
#[ignore = "needs parser"]
fn parsed_legacy_component() {
    parse_and_analyze(
        r#"<script>
	import { writable } from 'svelte/store';
	const store = writable(0);
	let n = 0;
	$: doubled = n * 2;
</script>
<input bind:value={n} />{$store}{doubled}"#,
        |r, a| {
            assert!(!a.runes);
            assert!(binding_named(a, a.instance.scope, "n").reassigned);
            assert_eq!(
                binding_named(a, a.instance.scope, "doubled").kind,
                BindingKind::LegacyReactive
            );
            assert_eq!(kind_of(a, idents(r, "$store")[0]), Some(BindingKind::StoreSub));
        },
    );
}
