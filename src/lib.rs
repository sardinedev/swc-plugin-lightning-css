#![allow(clippy::not_unsafe_ptr_arg_deref)]
mod utils;

use std::fmt::Debug;
use std::fs;

use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::{
    EmptyStmt, Expr, JSXAttr, JSXAttrName, JSXAttrValue, JSXExpr, MemberExpr, ModuleDecl,
    ModuleItem, Stmt,
};
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};
use swc_core::{
    common::util::take::Take,
    ecma::{
        ast::{ImportDecl, Program},
        transforms::testing::test,
        visit::{as_folder, FoldWith, VisitMut, VisitMutWith},
    },
};

#[derive(Default)]
struct TransformVisitor;

impl VisitMut for TransformVisitor {
    /*
     * Walk the AST and find all CSS Modules import declarations.
     * We mark and remove the module declaration from the AST.
     */
    fn visit_mut_import_decl(&mut self, node: &mut ImportDecl) {
        node.visit_mut_children_with(self);

        if node.specifiers.is_empty() {
            return;
        }

        if node.src.value.ends_with(".module.css") {
            // let mut css_module_map = get_css_module_mapping(&node.src.value.to_string());
            node.specifiers.take();
        }
    }

    fn visit_mut_jsx_attr(&mut self, attr: &mut JSXAttr) {
        attr.visit_mut_children_with(self);

        // check if the attribute name is "className"
        if let JSXAttrName::Ident(ident) = &attr.name {
            if ident.sym == *"className" {
                if let Some(JSXAttrValue::JSXExprContainer(container)) = &attr.value {
                    if let JSXExpr::Expr(expr) = &container.expr {
                        // Checks if we're just passing a propriety to the className, ie: className={styles.foo}
                        if let Expr::Member(member) = &**expr {
                            if let Expr::Ident(ident) = &*member.obj {
                                if ident.sym == *"styles" {
                                    let obj = &member.prop.as_ident().unwrap().sym.to_string();
                                    // println!("Found styles with pros: {:?}", obj);
                                }
                            }
                        }
                        // Checks if we're passing a string to the className, ie: className={`foo ${styles.foo}`}
                        if let Expr::Tpl(template) = &**expr {
                            for expr in &template.exprs {
                                if let Expr::Member(member) = &**expr {
                                    if let Expr::Ident(ident) = &*member.obj {
                                        if ident.sym == *"styles" {
                                            let obj =
                                                &member.prop.as_ident().unwrap().sym.to_string();
                                            println!("Found styles with pros: {:?}", obj);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Walk the ASt and finds import declarations that have been marked for removal.
    // We remove top level import declaration from the AST.
    fn visit_mut_module_item(&mut self, node: &mut ModuleItem) {
        node.visit_mut_children_with(self);

        if let ModuleItem::ModuleDecl(ModuleDecl::Import(import)) = node {
            if import.specifiers.is_empty() {
                *node = ModuleItem::Stmt(Stmt::Empty(EmptyStmt { span: DUMMY_SP }));
            }
        }
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

/*
 * Takes a relative path to a CSS file and returns an object with the mapping with the scoped class names.
 */
fn get_css_module_mapping(css_file_path: &String) -> serde_json::Value {
    // Replace the .css extension with .json
    let json_file = css_file_path.replace(".css", ".json").replace("./", "");

    let json_file_path = utils::find_file(json_file).unwrap();

    let data = fs::read_to_string(json_file_path).expect("Unable to read file");
    let json = serde_json::from_str(&data).expect("JSON does not have correct format.");
    return json;
}

#[plugin_transform]
pub fn process_transform(program: Program, _metadata: TransformPluginProgramMetadata) -> Program {
    program.fold_with(&mut as_folder(TransformVisitor))
}

test!(
    Default::default(),
    |_| as_folder(TransformVisitor),
    remove_import,
    r#"
    import styles from "./button.module.css";
    export const Button = () => (<button className={styles.button} id="3"/>);
    "#,
    r#"export const Button = ()=>{
        return h("button", {
            className: 'button'
        });
    };"#
);
