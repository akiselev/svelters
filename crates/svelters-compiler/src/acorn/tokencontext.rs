//! Port of acorn 8.19.0 `src/tokencontext.js`.
//!
//! The algorithm used to determine whether a regexp can appear at a
//! given point in the program is loosely based on sweet.js' approach.
//! See https://github.com/mozilla/sweet.js/wiki/design

use super::state::Parser;
use super::tokentype::TokenType as T;
use super::whitespace::has_line_break;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokContext {
    BStat,
    BExpr,
    BTmpl,
    PStat,
    PExpr,
    QTmpl,
    FStat,
    FExpr,
    FExprGen,
    FGen,
}

impl TokContext {
    pub fn token(self) -> &'static str {
        match self {
            TokContext::BStat | TokContext::BExpr => "{",
            TokContext::BTmpl => "${",
            TokContext::PStat | TokContext::PExpr => "(",
            TokContext::QTmpl => "`",
            TokContext::FStat | TokContext::FExpr | TokContext::FExprGen | TokContext::FGen => "function",
        }
    }
    pub fn is_expr(self) -> bool {
        matches!(self, TokContext::BExpr | TokContext::PExpr | TokContext::QTmpl | TokContext::FExpr | TokContext::FExprGen)
    }
    pub fn preserve_space(self) -> bool {
        self == TokContext::QTmpl
    }
    /// `override` is set (to `tryReadTemplateToken`) only for `q_tmpl`.
    pub fn has_override(self) -> bool {
        self == TokContext::QTmpl
    }
    pub fn generator(self) -> bool {
        matches!(self, TokContext::FExprGen | TokContext::FGen)
    }
}

impl<'a> Parser<'a> {
    pub fn initial_context(&self) -> Vec<TokContext> {
        vec![TokContext::BStat]
    }

    pub fn cur_context(&self) -> Option<TokContext> {
        self.context.last().copied()
    }

    pub fn brace_is_block(&self, prev_type: T) -> bool {
        let parent = self.cur_context();
        if parent == Some(TokContext::FExpr) || parent == Some(TokContext::FStat) {
            return true;
        }
        if prev_type == T::Colon && (parent == Some(TokContext::BStat) || parent == Some(TokContext::BExpr)) {
            return !parent.unwrap().is_expr();
        }

        // The check for `tt.name && exprAllowed` detects whether we are
        // after a `yield` or `of` construct. See the `updateContext` for
        // `tt.name`.
        if prev_type == T::Return || prev_type == T::Name && self.expr_allowed {
            return has_line_break(&self.input[self.last_tok_end as usize..self.start as usize]);
        }
        if matches!(prev_type, T::Else | T::Semi | T::Eof | T::ParenR | T::Arrow) {
            return true;
        }
        if prev_type == T::BraceL {
            return parent == Some(TokContext::BStat);
        }
        if matches!(prev_type, T::Var | T::Const | T::Name) {
            return false;
        }
        !self.expr_allowed
    }

    pub fn in_generator_context(&self) -> bool {
        for i in (1..self.context.len()).rev() {
            let context = self.context[i];
            if context.token() == "function" {
                return context.generator();
            }
        }
        false
    }

    pub fn update_context(&mut self, prev_type: T) {
        let type_ = self.type_;
        if type_.is_keyword() && prev_type == T::Dot {
            self.expr_allowed = false;
        } else if !self.run_update_context(type_, prev_type) {
            self.expr_allowed = type_.before_expr();
        }
    }

    // Used to handle edge cases when token context could not be inferred correctly during tokenization phase

    pub fn override_context(&mut self, token_ctx: TokContext) {
        if self.cur_context() != Some(token_ctx) {
            if let Some(last) = self.context.last_mut() {
                *last = token_ctx;
            }
        }
    }

    // Token-specific context update code. Returns false when the type has no `updateContext`.

    fn run_update_context(&mut self, type_: T, prev_type: T) -> bool {
        match type_ {
            T::ParenR | T::BraceR => {
                if self.context.len() == 1 {
                    self.expr_allowed = true;
                    return true;
                }
                let mut out = self.context.pop();
                if out == Some(TokContext::BStat) && self.cur_context().map(|c| c.token()) == Some("function") {
                    out = self.context.pop();
                }
                self.expr_allowed = !out.map(|o| o.is_expr()).unwrap_or(false);
            }
            T::BraceL => {
                let c = if self.brace_is_block(prev_type) { TokContext::BStat } else { TokContext::BExpr };
                self.context.push(c);
                self.expr_allowed = true;
            }
            T::DollarBraceL => {
                self.context.push(TokContext::BTmpl);
                self.expr_allowed = true;
            }
            T::ParenL => {
                let statement_parens = matches!(prev_type, T::If | T::For | T::With | T::While);
                self.context.push(if statement_parens { TokContext::PStat } else { TokContext::PExpr });
                self.expr_allowed = true;
            }
            T::IncDec => {
                // tokExprAllowed stays unchanged
            }
            T::Function | T::Class => {
                if prev_type.before_expr()
                    && prev_type != T::Else
                    && !(prev_type == T::Semi && self.cur_context() != Some(TokContext::PStat))
                    && !(prev_type == T::Return
                        && has_line_break(&self.input[self.last_tok_end as usize..self.start as usize]))
                    && !((prev_type == T::Colon || prev_type == T::BraceL)
                        && self.cur_context() == Some(TokContext::BStat))
                {
                    self.context.push(TokContext::FExpr);
                } else {
                    self.context.push(TokContext::FStat);
                }
                self.expr_allowed = false;
            }
            T::Colon => {
                if self.cur_context().map(|c| c.token()) == Some("function") {
                    self.context.pop();
                }
                self.expr_allowed = true;
            }
            T::BackQuote => {
                if self.cur_context() == Some(TokContext::QTmpl) {
                    self.context.pop();
                } else {
                    self.context.push(TokContext::QTmpl);
                }
                self.expr_allowed = false;
            }
            T::Star => {
                if prev_type == T::Function {
                    if let Some(last) = self.context.last_mut() {
                        if *last == TokContext::FExpr {
                            *last = TokContext::FExprGen;
                        } else {
                            *last = TokContext::FGen;
                        }
                    }
                }
                self.expr_allowed = true;
            }
            T::Name => {
                let mut allowed = false;
                if self.options.ecma_version >= 6 && prev_type != T::Dot {
                    let v = match &self.value {
                        super::state::TokVal::Str(s) => s.as_str(),
                        _ => "",
                    };
                    if v == "of" && !self.expr_allowed || v == "yield" && self.in_generator_context() {
                        allowed = true;
                    }
                }
                self.expr_allowed = allowed;
            }
            _ => return false,
        }
        true
    }
}
