#![allow(clippy::not_unsafe_ptr_arg_deref)]

use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::{EmptyStmt, ModuleDecl, ModuleItem, Stmt};
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};
use swc_core::{
    common::util::take::Take,
    ecma::{
        ast::{ImportDecl, Program},
        transforms::testing::test,
        visit::{as_folder, FoldWith, VisitMut, VisitMutWith},
    },
};

pub struct TransformVisitor;

/*
Walk the AST and find all CSS Modules import declarations.
We mark and remove the module declaration from the AST.
*/

impl VisitMut for TransformVisitor {
    fn visit_mut_import_decl(&mut self, node: &mut ImportDecl) {
        node.visit_mut_children_with(self);

        if node.specifiers.is_empty() {
            return;
        }

        if node.src.value.ends_with(".module.css") {
            node.specifiers.take();
        }
    }

    // Walk the ASt and finds import declarations that have been marked for removal.
    // We remove top level import declaration from the AST.
    fn visit_mut_module_item(&mut self, node: &mut ModuleItem) {
        node.visit_mut_children_with(self);

        if let ModuleItem::ModuleDecl(decl) = node {
            if let ModuleDecl::Import(import) = decl {
                if import.specifiers.is_empty() {
                    *node = ModuleItem::Stmt(Stmt::Empty(EmptyStmt { span: DUMMY_SP }));
                }
            }
        }
    }

    fn visit_mut_stmts(&mut self, stmts: &mut Vec<Stmt>) {
        stmts.visit_mut_children_with(self);

        // We remove `Stmt::Empty` from the statement list.
        // This is optional, but it's required if you don't want extra `;` in output.
        stmts.retain(|s| {
            // We use `matches` macro as this match is trivial.
            !matches!(s, Stmt::Empty(..))
        });
    }

    fn visit_mut_module_items(&mut self, stmts: &mut Vec<ModuleItem>) {
        stmts.visit_mut_children_with(self);

        // This is also required, because top-level statements are stored in `Vec<ModuleItem>`.
        stmts.retain(|s| {
            // We use `matches` macro as this match is trivial.
            !matches!(s, ModuleItem::Stmt(Stmt::Empty(..)))
        });
    }
}

#[plugin_transform]
pub fn process_transform(program: Program, _metadata: TransformPluginProgramMetadata) -> Program {
    program.fold_with(&mut as_folder(TransformVisitor))
}

test!(
    Default::default(),
    |_| as_folder(TransformVisitor),
    remove_import,
    r#"import styles from "./button.module.css";"#,
    r#""#
);
