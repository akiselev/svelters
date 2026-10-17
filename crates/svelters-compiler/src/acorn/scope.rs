//! Port of acorn 8.19.0 `src/scope.js`.
//!
//! SVELTERS: the per-scope name lists are hash sets (acorn uses arrays + `indexOf`, which is
//! quadratic for large scopes); `lexical[0]` is kept as `lexical_first`.

use std::collections::HashSet;

use super::scopeflags::*;
use super::state::{PResult, Parser};

pub struct Scope {
    pub flags: u32,
    // A list of var-declared names in the current lexical scope
    pub var: HashSet<String>,
    // A list of lexically-declared names in the current lexical scope
    pub lexical: HashSet<String>,
    pub lexical_first: Option<String>,
    // A list of lexically-declared FunctionDeclaration names in the current lexical scope
    pub functions: HashSet<String>,
}

impl Scope {
    fn new(flags: u32) -> Scope {
        Scope { flags, var: HashSet::new(), lexical: HashSet::new(), lexical_first: None, functions: HashSet::new() }
    }

    fn push_lexical(&mut self, name: &str) {
        if self.lexical_first.is_none() {
            self.lexical_first = Some(name.to_string());
        }
        self.lexical.insert(name.to_string());
    }
}

// The functions in this module keep track of declared variables in the current scope in order to detect duplicate variable names.

impl<'a> Parser<'a> {
    pub fn enter_scope(&mut self, flags: u32) {
        self.scope_stack.push(Scope::new(flags))
    }

    pub fn exit_scope(&mut self) {
        self.scope_stack.pop();
    }

    // The spec says:
    // > At the top level of a function, or script, function declarations are
    // > treated like var declarations rather than like lexical declarations.
    pub fn treat_functions_as_var_in_scope(&self, scope: &Scope) -> bool {
        (scope.flags & SCOPE_FUNCTION) != 0 || !self.in_module && (scope.flags & SCOPE_TOP) != 0
    }

    pub fn declare_name(&mut self, name: &str, binding_type: u32, pos: u32) -> PResult<()> {
        let mut redeclared = false;
        if binding_type == BIND_LEXICAL {
            let in_module = self.in_module;
            let scope = self.scope_stack.last_mut().unwrap();
            redeclared = scope.lexical.contains(name) || scope.functions.contains(name) || scope.var.contains(name);
            scope.push_lexical(name);
            if in_module && (scope.flags & SCOPE_TOP) != 0 {
                self.undefined_exports.delete(name);
            }
        } else if binding_type == BIND_SIMPLE_CATCH {
            let scope = self.scope_stack.last_mut().unwrap();
            scope.push_lexical(name);
        } else if binding_type == BIND_FUNCTION {
            let treat = self.treat_functions_as_var();
            let scope = self.scope_stack.last_mut().unwrap();
            if treat {
                redeclared = scope.lexical.contains(name);
            } else {
                redeclared = scope.lexical.contains(name) || scope.var.contains(name);
            }
            scope.functions.insert(name.to_string());
        } else {
            let mut i = self.scope_stack.len();
            while i > 0 {
                i -= 1;
                let treat = self.treat_functions_as_var_in_scope(&self.scope_stack[i]);
                let in_module = self.in_module;
                let scope = &mut self.scope_stack[i];
                if scope.lexical.contains(name)
                    && !((scope.flags & SCOPE_SIMPLE_CATCH) != 0 && scope.lexical_first.as_deref() == Some(name))
                    || !treat && scope.functions.contains(name)
                {
                    redeclared = true;
                    break;
                }
                scope.var.insert(name.to_string());
                let flags = scope.flags;
                if in_module && (flags & SCOPE_TOP) != 0 {
                    self.undefined_exports.delete(name);
                }
                if flags & SCOPE_VAR != 0 {
                    break;
                }
            }
        }
        if redeclared {
            return self.raise_recoverable(pos, format!("Identifier '{}' has already been declared", name));
        }
        Ok(())
    }

    pub fn check_local_export(&mut self, name: &str, start: u32) {
        // scope.functions must be empty as Module code is always strict.
        let top = &self.scope_stack[0];
        if !top.lexical.contains(name) && !top.var.contains(name) {
            self.undefined_exports.set(name, start);
        }
    }

    pub fn current_scope(&self) -> &Scope {
        self.scope_stack.last().unwrap()
    }

    pub fn current_var_scope(&self) -> &Scope {
        for scope in self.scope_stack.iter().rev() {
            if scope.flags & (SCOPE_VAR | SCOPE_CLASS_FIELD_INIT | SCOPE_CLASS_STATIC_BLOCK) != 0 {
                return scope;
            }
        }
        &self.scope_stack[0]
    }

    // Could be useful for `this`, `new.target`, `super()`, `super.property`, and `super[property]`.
    pub fn current_this_scope(&self) -> &Scope {
        for scope in self.scope_stack.iter().rev() {
            if scope.flags & (SCOPE_VAR | SCOPE_CLASS_FIELD_INIT | SCOPE_CLASS_STATIC_BLOCK) != 0
                && (scope.flags & SCOPE_ARROW) == 0
            {
                return scope;
            }
        }
        &self.scope_stack[0]
    }
}
